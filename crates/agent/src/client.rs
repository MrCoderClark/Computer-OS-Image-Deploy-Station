use anyhow::{Context, Result};
use imaging_common::api::*;

pub struct ServerClient {
    http: reqwest::Client,
    base_url: String,
    session_token: Option<String>,
}

impl ServerClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            base_url: format!("http://{host}:{port}"),
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

    pub async fn authenticate(&mut self, machine_name: &str) -> Result<AuthResponse> {
        let req = AuthRequest {
            machine_name: machine_name.into(),
            enrollment_key: None,
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
