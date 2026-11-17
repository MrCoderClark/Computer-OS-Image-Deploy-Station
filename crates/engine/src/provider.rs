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
