use anyhow::Result;
use std::path::{Path, PathBuf};
use imaging_common::config::{OperatingMode, SuiteConfig};

pub trait ImageProvider: Send + Sync {
    fn wim_path(&self) -> &Path;
    fn image_index(&self) -> u32;
    fn description(&self) -> String;
}

pub struct LocalDiskProvider {
    path: PathBuf,
    index: u32,
}

impl LocalDiskProvider {
    pub fn from_config(config: &SuiteConfig) -> Result<Self> {
        let wim_path = config
            .image
            .as_ref()
            .and_then(|i| i.wim_path.as_deref())
            .unwrap_or(r"X:\imaging\deploy.wim");

        Ok(Self {
            path: PathBuf::from(wim_path),
            index: 1,
        })
    }
}

impl ImageProvider for LocalDiskProvider {
    fn wim_path(&self) -> &Path {
        &self.path
    }

    fn image_index(&self) -> u32 {
        self.index
    }

    fn description(&self) -> String {
        format!("Local: {}", self.path.display())
    }
}

pub struct NetworkProvider {
    cached_path: PathBuf,
    index: u32,
}

impl NetworkProvider {
    pub fn new(cached_path: PathBuf, index: u32) -> Self {
        Self { cached_path, index }
    }
}

impl ImageProvider for NetworkProvider {
    fn wim_path(&self) -> &Path {
        &self.cached_path
    }

    fn image_index(&self) -> u32 {
        self.index
    }

    fn description(&self) -> String {
        format!("Network (cached): {}", self.cached_path.display())
    }
}

pub fn create_provider(config: &SuiteConfig) -> Result<Box<dyn ImageProvider>> {
    match config.mode {
        OperatingMode::Offline => {
            let provider = LocalDiskProvider::from_config(config)?;
            tracing::info!("Image provider: {}", provider.description());
            Ok(Box::new(provider))
        }
        OperatingMode::Online => {
            let temp = std::env::temp_dir().join("deploystation_download.wim");
            let provider = NetworkProvider::new(temp, 1);
            tracing::info!("Image provider: {}", provider.description());
            Ok(Box::new(provider))
        }
        OperatingMode::Hybrid => {
            let local_path = config
                .image
                .as_ref()
                .and_then(|i| i.wim_path.as_deref())
                .unwrap_or(r"X:\imaging\deploy.wim");

            if Path::new(local_path).exists() {
                let provider = LocalDiskProvider::from_config(config)?;
                tracing::info!("Hybrid → local image available: {}", provider.description());
                Ok(Box::new(provider))
            } else {
                let temp = std::env::temp_dir().join("deploystation_download.wim");
                let provider = NetworkProvider::new(temp, 1);
                tracing::info!("Hybrid → falling back to network: {}", provider.description());
                Ok(Box::new(provider))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use imaging_common::config::*;

    fn make_config(mode: OperatingMode, wim_path: Option<&str>) -> SuiteConfig {
        let image = match mode {
            OperatingMode::Online => None,
            _ => Some(ImageMeta {
                name: "test".into(),
                os_family: OsFamily::Win11,
                captured_from: None,
                build_date: None,
                size_bytes: None,
                sha256: None,
                wim_path: wim_path.map(|s| s.to_string()),
            }),
        };
        let server = match mode {
            OperatingMode::Offline => None,
            _ => Some(ServerConfig {
                discovery: DiscoveryMethod::Fixed,
                host: Some("10.0.0.1".into()),
                port: 7443,
            }),
        };
        SuiteConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            mode,
            server,
            capture_target: None,
            image,
            enrollment_key: None,
            built_by: "test".into(),
            built_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn offline_uses_local_provider() {
        let cfg = make_config(OperatingMode::Offline, None);
        let provider = create_provider(&cfg).unwrap();
        assert_eq!(provider.wim_path(), Path::new(r"X:\imaging\deploy.wim"));
        assert_eq!(provider.image_index(), 1);
        assert!(provider.description().contains("Local"));
    }

    #[test]
    fn offline_custom_wim_path() {
        let cfg = make_config(OperatingMode::Offline, Some(r"D:\images\custom.wim"));
        let provider = create_provider(&cfg).unwrap();
        assert_eq!(provider.wim_path(), Path::new(r"D:\images\custom.wim"));
    }

    #[test]
    fn online_uses_network_provider() {
        let cfg = make_config(OperatingMode::Online, None);
        let provider = create_provider(&cfg).unwrap();
        assert!(provider.description().contains("Network"));
        assert_eq!(provider.image_index(), 1);
    }

    #[test]
    fn network_provider_manual() {
        let p = NetworkProvider::new(PathBuf::from(r"C:\temp\download.wim"), 2);
        assert_eq!(p.wim_path(), Path::new(r"C:\temp\download.wim"));
        assert_eq!(p.image_index(), 2);
        assert!(p.description().contains("Network"));
    }

    #[test]
    fn local_provider_description() {
        let cfg = make_config(OperatingMode::Offline, Some(r"X:\test.wim"));
        let p = LocalDiskProvider::from_config(&cfg).unwrap();
        assert!(p.description().starts_with("Local:"));
        assert!(p.description().contains("test.wim"));
    }
}
