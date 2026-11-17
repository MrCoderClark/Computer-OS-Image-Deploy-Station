use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverPack {
    pub id: Uuid,
    pub name: String,
    pub hardware_model: String,
    pub os_family: crate::config::OsFamily,
    pub path: String,
    pub driver_count: u32,
    pub size_bytes: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
