use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageListResponse {
    pub images: Vec<ImageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSummary {
    pub id: Uuid,
    pub name: String,
    pub os_family: crate::config::OsFamily,
    pub captured_from: Option<String>,
    pub build_date: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub machine_name: String,
    pub enrollment_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub session_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClaims {
    pub machine_name: String,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub image_id: Uuid,
    pub transfer_id: Uuid,
    pub offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployStatusUpdate {
    pub job_id: Uuid,
    pub status: crate::types::DeployStatus,
    pub progress_pct: u8,
    pub message: String,
}
