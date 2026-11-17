mod app;
mod client;
mod ui;

use anyhow::Result;
use eframe::egui;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("imaging_agent=debug")
        .init();

    let config = load_config();

    if let Some(ref cfg) = config {
        match cfg.validate() {
            Ok(()) => tracing::info!("Config valid: mode={}, built_by={}", cfg.mode, cfg.built_by),
            Err(errors) => {
                for err in &errors {
                    tracing::error!("Config validation error: {err}");
                }
                tracing::warn!("Continuing with invalid config — some features may not work");
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Windows Imaging Agent")
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    tracing::info!("Starting Windows Imaging Agent");

    eframe::run_native(
        "Windows Imaging Agent",
        options,
        Box::new(move |_cc| Ok(Box::new(app::AgentApp::new(config)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

fn load_config() -> Option<imaging_common::config::SuiteConfig> {
    let candidates = [
        std::path::PathBuf::from(r"X:\imaging\config.json"),
        std::path::PathBuf::from("config.json"),
    ];

    for path in &candidates {
        if path.exists() {
            tracing::info!("Loading config from {}", path.display());
            match std::fs::read_to_string(path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(config) => return Some(config),
                    Err(e) => tracing::error!("Failed to parse {}: {e}", path.display()),
                },
                Err(e) => tracing::error!("Failed to read {}: {e}", path.display()),
            }
        }
    }

    tracing::warn!("No config.json found — running in standalone mode");
    None
}
