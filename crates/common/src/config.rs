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

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_online_config() -> SuiteConfig {
        SuiteConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            mode: OperatingMode::Online,
            server: Some(ServerConfig {
                discovery: DiscoveryMethod::Fixed,
                host: Some("10.0.0.1".into()),
                port: 7443,
            }),
            capture_target: None,
            image: None,
            enrollment_key: None,
            built_by: "test".into(),
            built_at: chrono::Utc::now(),
        }
    }

    fn minimal_offline_config() -> SuiteConfig {
        SuiteConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            mode: OperatingMode::Offline,
            server: None,
            capture_target: None,
            image: Some(ImageMeta {
                name: "test-image".into(),
                os_family: OsFamily::Win11,
                captured_from: None,
                build_date: None,
                size_bytes: None,
                sha256: None,
                wim_path: None,
            }),
            enrollment_key: Some("test-key-123".into()),
            built_by: "test".into(),
            built_at: chrono::Utc::now(),
        }
    }

    fn minimal_hybrid_config() -> SuiteConfig {
        SuiteConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            mode: OperatingMode::Hybrid,
            server: Some(ServerConfig {
                discovery: DiscoveryMethod::Fixed,
                host: Some("10.0.0.1".into()),
                port: 7443,
            }),
            capture_target: None,
            image: Some(ImageMeta {
                name: "fallback-image".into(),
                os_family: OsFamily::Win11,
                captured_from: None,
                build_date: None,
                size_bytes: None,
                sha256: None,
                wim_path: None,
            }),
            enrollment_key: None,
            built_by: "test".into(),
            built_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn valid_online_config() {
        assert!(minimal_online_config().validate().is_ok());
    }

    #[test]
    fn valid_offline_config() {
        assert!(minimal_offline_config().validate().is_ok());
    }

    #[test]
    fn valid_hybrid_config() {
        assert!(minimal_hybrid_config().validate().is_ok());
    }

    #[test]
    fn online_requires_server_block() {
        let mut cfg = minimal_online_config();
        cfg.server = None;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("server")));
    }

    #[test]
    fn offline_requires_image_block() {
        let mut cfg = minimal_offline_config();
        cfg.image = None;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("image")));
    }

    #[test]
    fn offline_requires_enrollment_key() {
        let mut cfg = minimal_offline_config();
        cfg.enrollment_key = None;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("enrollment_key")));
    }

    #[test]
    fn hybrid_requires_both_server_and_image() {
        let mut cfg = minimal_hybrid_config();
        cfg.server = None;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("server")));

        let mut cfg = minimal_hybrid_config();
        cfg.image = None;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("image")));
    }

    #[test]
    fn wrong_schema_version_rejected() {
        let mut cfg = minimal_online_config();
        cfg.schema_version = 999;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("schema_version")));
    }

    #[test]
    fn empty_built_by_rejected() {
        let mut cfg = minimal_online_config();
        cfg.built_by = String::new();
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("built_by")));
    }

    #[test]
    fn fixed_discovery_requires_host() {
        let mut cfg = minimal_online_config();
        cfg.server.as_mut().unwrap().host = None;
        let errors = cfg.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("host")));
    }

    #[test]
    fn dns_discovery_no_host_ok() {
        let mut cfg = minimal_online_config();
        cfg.server.as_mut().unwrap().discovery = DiscoveryMethod::Dns;
        cfg.server.as_mut().unwrap().host = None;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn needs_server_flags() {
        assert!(minimal_online_config().needs_server());
        assert!(minimal_hybrid_config().needs_server());
        assert!(!minimal_offline_config().needs_server());
    }

    #[test]
    fn has_local_image_flags() {
        assert!(!minimal_online_config().has_local_image());
        assert!(minimal_offline_config().has_local_image());
        assert!(minimal_hybrid_config().has_local_image());
    }

    #[test]
    fn config_roundtrip_json() {
        let cfg = minimal_hybrid_config();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: SuiteConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mode, OperatingMode::Hybrid);
        assert_eq!(parsed.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(parsed.server.is_some());
        assert!(parsed.image.is_some());
    }

    #[test]
    fn mode_deserializes_lowercase() {
        let val: OperatingMode = serde_json::from_str("\"online\"").unwrap();
        assert_eq!(val, OperatingMode::Online);
        let val: OperatingMode = serde_json::from_str("\"offline\"").unwrap();
        assert_eq!(val, OperatingMode::Offline);
        let val: OperatingMode = serde_json::from_str("\"hybrid\"").unwrap();
        assert_eq!(val, OperatingMode::Hybrid);
    }

    #[test]
    fn os_family_display() {
        assert_eq!(OsFamily::Win10.to_string(), "Windows 10");
        assert_eq!(OsFamily::Win11.to_string(), "Windows 11");
    }

    #[test]
    fn multiple_errors_accumulated() {
        let cfg = SuiteConfig {
            schema_version: 999,
            mode: OperatingMode::Online,
            server: None,
            capture_target: None,
            image: None,
            enrollment_key: None,
            built_by: String::new(),
            built_at: chrono::Utc::now(),
        };
        let errors = cfg.validate().unwrap_err();
        assert!(errors.len() >= 3);
    }
}
