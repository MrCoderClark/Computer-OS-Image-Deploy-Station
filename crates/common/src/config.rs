use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

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
    #[serde(default)]
    pub enrollment_key: Option<String>,
    pub built_by: String,
    pub built_at: chrono::DateTime<chrono::Utc>,
}

impl SuiteConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.schema_version != CURRENT_SCHEMA_VERSION {
            errors.push(format!(
                "Unsupported schema_version {}, expected {}",
                self.schema_version, CURRENT_SCHEMA_VERSION
            ));
        }

        match self.mode {
            OperatingMode::Online => {
                if self.server.is_none() {
                    errors.push("Online mode requires a 'server' block".into());
                }
            }
            OperatingMode::Offline => {
                if self.image.is_none() {
                    errors.push("Offline mode requires an 'image' block".into());
                }
            }
            OperatingMode::Hybrid => {
                if self.server.is_none() {
                    errors.push("Hybrid mode requires a 'server' block".into());
                }
                if self.image.is_none() {
                    errors.push("Hybrid mode requires an 'image' block (fallback)".into());
                }
            }
        }

        if let Some(server) = &self.server {
            if server.discovery == DiscoveryMethod::Fixed && server.host.is_none() {
                errors.push("Fixed discovery requires 'host' in server config".into());
            }
        }

        if self.mode == OperatingMode::Offline && self.enrollment_key.is_none() {
            errors.push("Offline mode requires an 'enrollment_key' for authentication".into());
        }

        if self.built_by.is_empty() {
            errors.push("'built_by' must not be empty".into());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn needs_server(&self) -> bool {
        matches!(self.mode, OperatingMode::Online | OperatingMode::Hybrid)
    }

    pub fn has_local_image(&self) -> bool {
        matches!(self.mode, OperatingMode::Offline | OperatingMode::Hybrid) && self.image.is_some()
    }
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
    #[serde(default)]
    pub wim_path: Option<String>,
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
