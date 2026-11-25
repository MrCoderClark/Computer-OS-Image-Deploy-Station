use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRecord {
    pub id: Uuid,
    pub name: String,
    pub os_family: crate::config::OsFamily,
    pub captured_from: Option<String>,
    pub build_date: chrono::NaiveDate,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Which image index to apply inside the WIM. Multi-edition install.wim
    /// files hold several (Home, Pro, Education, ...); captured WIMs usually
    /// have one. Defaults to 1, which is what single-index captures and most
    /// install media's first edition want.
    #[serde(default = "default_wim_index")]
    pub wim_index: u32,
}

pub fn default_wim_index() -> u32 { 1 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployStatus {
    Pending,
    Partitioning,
    Applying,
    Injecting,
    Configuring,
    Rebooting,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployJob {
    pub id: Uuid,
    pub machine_name: String,
    pub image_id: Uuid,
    pub status: DeployStatus,
    pub progress_pct: u8,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub started_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub detail: Option<String>,
    pub level: AuditLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureManifest {
    pub id: Uuid,
    pub image_name: String,
    pub os_family: crate::config::OsFamily,
    pub source_machine: String,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
    pub sha256: String,
    pub unc_path: String,
}
