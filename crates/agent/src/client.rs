use anyhow::{Context, Result};
use imaging_common::api::*;

pub struct ServerClient {
    http: reqwest::Client,
    base_url: String,
    session_token: Option<String>,
}

impl ServerClient {
    /// Create a new client. When `cert_pem` is supplied (from the suite
    /// config), the agent pins to that exact cert as its sole trust anchor.
    /// When it is absent, the agent falls back to accept-any-cert with a
    /// loud warning (Phase 17 behavior; only acceptable for first-boot
    /// bootstrap on a trusted LAN).
    pub fn new(host: &str, port: u16, cert_pem: Option<&str>) -> Self {
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10));
        builder = match cert_pem {
            Some(pem) => match reqwest::Certificate::from_pem(pem.as_bytes()) {
                Ok(cert) => {
                    tracing::info!("TLS pinned to server cert from suite config");
                    builder.add_root_certificate(cert)
                }
                Err(e) => {
                    tracing::error!("Pinned cert_pem in suite config failed to parse ({e}); falling back to accept-any");
                    builder.danger_accept_invalid_certs(true)
                }
            },
            None => {
                tracing::warn!("No pinned cert in suite config; TLS verification DISABLED. Rebuild USB after `trust_server` in admin to enable pinning.");
                builder.danger_accept_invalid_certs(true)
            }
        };
        let http = builder.build().expect("failed to build HTTP client");
        Self {
            http,
            base_url: format!("https://{host}:{port}"),
            session_token: None,
        }
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let resp = self
            .http
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .context("failed to reach server")?
            .error_for_status()
            .context("server returned error")?
            .json::<HealthResponse>()
            .await
            .context("failed to parse health response")?;
        Ok(resp)
    }

    pub async fn authenticate(&mut self, machine_name: &str, enrollment_key: Option<&str>) -> Result<AuthResponse> {
        let req = AuthRequest {
            machine_name: machine_name.into(),
            enrollment_key: enrollment_key.map(|s| s.to_string()),
        };

        let resp = self
            .http
            .post(format!("{}/api/auth", self.base_url))
            .json(&req)
            .send()
            .await
            .context("auth request failed")?
            .error_for_status()
            .context("auth returned error")?
            .json::<AuthResponse>()
            .await
            .context("failed to parse auth response")?;

        self.session_token = Some(resp.session_token.clone());
        Ok(resp)
    }

    pub async fn list_images(&self) -> Result<Vec<ImageSummary>> {
        let token = self
            .session_token
            .as_deref()
            .context("not authenticated")?;

        let resp = self
            .http
            .get(format!("{}/api/images", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .context("image list request failed")?
            .error_for_status()
            .context("image list returned error")?
            .json::<ImageListResponse>()
            .await
            .context("failed to parse image list")?;

        Ok(resp.images)
    }

    #[allow(dead_code)]
    pub async fn report_deploy_status(
        &self,
        update: &imaging_common::api::DeployStatusUpdate,
    ) -> Result<()> {
        let token = self
            .session_token
            .as_deref()
            .context("not authenticated")?;

        self.http
            .post(format!("{}/api/deploy/status", self.base_url))
            .bearer_auth(token)
            .json(update)
            .send()
            .await
            .context("deploy status report failed")?
            .error_for_status()
            .context("deploy status report returned error")?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn reconcile_capture(
        &self,
        manifest: &imaging_common::types::CaptureManifest,
    ) -> Result<()> {
        let token = self
            .session_token
            .as_deref()
            .context("not authenticated")?;

        self.http
            .post(format!("{}/api/captures/reconcile", self.base_url))
            .bearer_auth(token)
            .json(manifest)
            .send()
            .await
            .context("capture reconciliation failed")?
            .error_for_status()
            .context("capture reconciliation returned error")?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_authenticated(&self) -> bool {
        self.session_token.is_some()
    }

    /// Download an image to `dest`, resuming from whatever bytes are already
    /// on disk there. Sends `Range: bytes=OFFSET-` when the partial file is
    /// non-empty; a cold start just sends GET.
    ///
    /// On completion, SHA-256-verifies the full file against
    /// `expected_sha256` (when provided). The server returns the sha256 as
    /// a strong ETag too, but we verify by hashing the assembled bytes so a
    /// range mis-stitch is caught.
    ///
    /// `progress` is called with `(downloaded_bytes, total_bytes)` after
    /// each chunk. Both numbers include any pre-existing bytes from a
    /// prior attempt.
    pub async fn download_image(
        &self,
        image_id: uuid::Uuid,
        dest: &std::path::Path,
        expected_sha256: Option<&str>,
        mut progress: impl FnMut(u64, u64) + Send,
    ) -> Result<()> {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let token = self.session_token.as_deref()
            .context("authenticate() before download_image()")?;
        let transfer_id = uuid::Uuid::new_v4();
        let url = format!("{}/api/images/{image_id}/download?transfer_id={transfer_id}", self.base_url);

        // How many bytes are already on disk from a prior attempt?
        let existing = tokio::fs::metadata(dest).await.map(|m| m.len()).unwrap_or(0);

        let mut req = self.http.get(&url)
            .bearer_auth(token)
            .timeout(std::time::Duration::from_secs(4 * 3600));
        if existing > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={existing}-"));
            tracing::info!("Resuming download of {image_id} from byte {existing}");
        } else {
            tracing::info!("Starting download of {image_id}");
        }

        let resp = req.send().await.context("send download request")?;
        let status = resp.status();
        if !status.is_success() {
            // 416 Range Not Satisfiable = we already have the whole file (or more).
            // Fall through to the sha256 check; if the file is complete and
            // intact, we're done.
            if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                tracing::info!("Server says existing file is already complete; verifying");
            } else {
                anyhow::bail!("server returned {status} for download");
            }
        } else {
            // Compute the total expected size from Content-Range (ranged
            // reply) or Content-Length (full reply) + any existing bytes.
            let total = match resp.headers().get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_content_range_total)
            {
                Some(t) => t,
                None => resp.content_length().map(|c| existing + c).unwrap_or(0),
            };

            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(existing > 0)
                .write(true)
                .truncate(existing == 0)
                .open(dest)
                .await
                .with_context(|| format!("open {} for append", dest.display()))?;

            let mut downloaded = existing;
            progress(downloaded, total);
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context("stream chunk")?;
                file.write_all(&chunk).await.context("write chunk")?;
                downloaded += chunk.len() as u64;
                progress(downloaded, total);
            }
            file.flush().await.context("flush")?;
        }

        // SHA-256 verify over whatever is on disk now, regardless of whether
        // we just appended or the file was already complete.
        if let Some(expected) = expected_sha256 {
            tracing::info!("Verifying SHA-256 of {}", dest.display());
            let actual = hash_file_sha256(dest).await
                .with_context(|| format!("hash {}", dest.display()))?;
            if !actual.eq_ignore_ascii_case(expected) {
                anyhow::bail!("sha256 mismatch: got {actual}, expected {expected}");
            }
        }

        Ok(())
    }
}

fn parse_content_range_total(hdr: &str) -> Option<u64> {
    // `bytes 100-199/12345`  ->  12345
    let rest = hdr.strip_prefix("bytes ")?;
    let slash = rest.rfind('/')?;
    let tail = &rest[slash + 1..];
    if tail == "*" { None } else { tail.parse().ok() }
}

async fn hash_file_sha256(path: &std::path::Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];  // 1 MiB
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
