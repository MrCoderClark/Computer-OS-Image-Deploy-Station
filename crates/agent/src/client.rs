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
}
