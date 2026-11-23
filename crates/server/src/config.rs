//! Server configuration loaded from `server.toml`.
//!
//! Lookup order:
//!   1. Path from `--config <file>` CLI flag
//!   2. `%ProgramData%\DeployStation\server.toml` (installed-service default)
//!   3. `server.toml` next to the executable
//!   4. Built-in defaults

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "Config::default_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "Config::default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "Config::default_log_filter")]
    pub log_filter: String,
    #[serde(default)]
    pub log_dir: Option<PathBuf>,
    #[serde(default)]
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// true = HTTPS with the cert at cert_path/key_path (auto-generated
    /// self-signed if missing). false = plain HTTP.
    #[serde(default = "TlsConfig::default_enabled")]
    pub enabled: bool,
    #[serde(default = "TlsConfig::default_cert_path")]
    pub cert_path: PathBuf,
    #[serde(default = "TlsConfig::default_key_path")]
    pub key_path: PathBuf,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled:   Self::default_enabled(),
            cert_path: Self::default_cert_path(),
            key_path:  Self::default_key_path(),
        }
    }
}

impl TlsConfig {
    fn default_enabled() -> bool { true }
    fn default_cert_path() -> PathBuf { PathBuf::from("tls").join("server.crt") }
    fn default_key_path()  -> PathBuf { PathBuf::from("tls").join("server.key") }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: Self::default_bind_addr(),
            db_path:   Self::default_db_path(),
            log_filter: Self::default_log_filter(),
            log_dir: None,
            tls: TlsConfig::default(),
        }
    }
}

impl Config {
    fn default_bind_addr() -> String { "0.0.0.0:7443".to_string() }
    fn default_db_path()   -> PathBuf { PathBuf::from("imaging-server.db") }
    fn default_log_filter() -> String { "imaging_server=info,tower_http=info".to_string() }

    /// Load config using the lookup order above. Returns defaults when no
    /// file is found; returns an error only if a file was found but could
    /// not be parsed.
    pub fn load(cli_path: Option<&Path>) -> anyhow::Result<Self> {
        if let Some(p) = cli_path {
            return Self::from_file(p);
        }
        for candidate in Self::search_paths() {
            if candidate.is_file() {
                tracing::info!("Loading config from {}", candidate.display());
                return Self::from_file(&candidate);
            }
        }
        tracing::info!("No server.toml found; using built-in defaults");
        Ok(Self::default())
    }

    fn from_file(p: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(p)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", p.display()))?;
        let cfg: Self = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parsing {}: {e}", p.display()))?;
        Ok(cfg)
    }

    fn search_paths() -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(pd) = std::env::var_os("ProgramData") {
            out.push(PathBuf::from(pd).join("DeployStation").join("server.toml"));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                out.push(dir.join("server.toml"));
            }
        }
        out
    }

    /// Resolve `db_path` and `log_dir` to absolute paths. Relative paths are
    /// taken against `%ProgramData%\DeployStation\` when the directory exists
    /// (i.e. installed service), otherwise against the exe's directory.
    pub fn resolve_paths(&mut self) {
        let base = Self::state_base();
        if self.db_path.is_relative() {
            self.db_path = base.join(&self.db_path);
        }
        if let Some(ref ld) = self.log_dir {
            if ld.is_relative() {
                self.log_dir = Some(base.join(ld));
            }
        }
        if self.tls.cert_path.is_relative() {
            self.tls.cert_path = base.join(&self.tls.cert_path);
        }
        if self.tls.key_path.is_relative() {
            self.tls.key_path = base.join(&self.tls.key_path);
        }
    }

    fn state_base() -> PathBuf {
        if let Some(pd) = std::env::var_os("ProgramData") {
            let p = PathBuf::from(pd).join("DeployStation");
            if p.is_dir() {
                return p;
            }
        }
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
