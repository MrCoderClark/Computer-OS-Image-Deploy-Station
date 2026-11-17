use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteConfig {
    pub schema_version: u32,
    pub mode: OperatingMode,
    #[serde(default)]
    pub server: Option<ServerConfig>,
    #[serde(default)]
    pub capture_target: Option<CaptureTarget>,
    #[serde(default)]
    pub image: Option<ImageMeta>,
    pub built_by: String,
    pub built_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperatingMode {
    Online,
    Offline,
    Hybrid,
}

impl std::fmt::Display for OperatingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Online => write!(f, "Online"),
            Self::Offline => write!(f, "Offline"),
            Self::Hybrid => write!(f, "Hybrid"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub discovery: DiscoveryMethod,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    7443
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryMethod {
    Dns,
    Dhcp,
    Fixed,
    Broadcast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTarget {
    #[serde(rename = "type")]
    pub target_type: String,
    pub unc_path: String,
    pub account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMeta {
    pub name: String,
    pub os_family: OsFamily,
    #[serde(default)]
    pub captured_from: Option<String>,
    #[serde(default)]
    pub build_date: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsFamily {
    Win10,
    Win11,
}

impl std::fmt::Display for OsFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Win10 => write!(f, "Windows 10"),
            Self::Win11 => write!(f, "Windows 11"),
        }
    }
}
