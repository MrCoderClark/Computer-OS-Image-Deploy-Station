mod db;
mod state;
mod routes;
mod config;

#[cfg(windows)]
mod service;

use anyhow::Result;
use axum::Router;
use tracing::info;

use crate::config::Config;

#[cfg(not(windows))]
fn main() -> Result<()> {
    // Non-Windows: only console mode exists.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg_path = parse_config_flag(&args);
    let mut cfg = Config::load(cfg_path.as_deref())?;
    cfg.resolve_paths();
    init_logging(&cfg);
    tokio::runtime::Runtime::new()?.block_on(run_server(cfg, futures_shutdown()))
}

#[cfg(windows)]
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("install") => {
            service::install()
        }
        Some("uninstall") => {
            service::uninstall()
        }
        Some("--service") => {
            // Invoked by SCM. Logging will be set up inside service_main
            // once the config loads. Any pre-config diagnostics go to stderr,
            // which the service wrapper ignores -- that's fine.
            service::run()
        }
        _ => {
            // Console mode.
            let cfg_path = parse_config_flag(&args);
            let mut cfg = Config::load(cfg_path.as_deref())?;
            cfg.resolve_paths();
            init_logging(&cfg);
            tokio::runtime::Runtime::new()?.block_on(run_server(cfg, futures_shutdown()))
        }
    }
}

fn parse_config_flag(args: &[String]) -> Option<std::path::PathBuf> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == "--config" || a == "-c" {
            if let Some(p) = iter.next() {
                return Some(std::path::PathBuf::from(p));
            }
        }
    }
    None
}

fn init_logging(cfg: &Config) {
    use tracing_subscriber::fmt;
    let filter = cfg.log_filter.clone();
    if let Some(dir) = cfg.log_dir.as_ref() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("Cannot create log dir {}: {e}", dir.display());
        }
        let file = tracing_appender::rolling::daily(dir, "imaging-server.log");
        fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .init();
    } else {
        fmt().with_env_filter(filter).init();
    }
}

/// Run the HTTP server until `shutdown` resolves.
pub async fn run_server<F>(cfg: Config, shutdown: F) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if let Some(parent) = cfg.db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let database = db::Database::open(&cfg.db_path)?;
    let app_state = state::AppState::new(database);

    let needs_seed = {
        let db = app_state.db.lock().await;
        db.list_images().map(|imgs| imgs.is_empty()).unwrap_or(true)
    };
    if needs_seed {
        let state_clone = app_state.clone();
        tokio::task::spawn_blocking(move || state_clone.seed_sample_images()).await?;
        info!("Seeded sample images into database");
    }

    let app = Router::new()
        .merge(routes::api_routes())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    info!("Imaging server listening on {} (db: {})", cfg.bind_addr, cfg.db_path.display());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    info!("Imaging server stopped");
    Ok(())
}

/// Ctrl-C / SIGTERM wait, for console mode. Never resolves when running as
/// a service -- the service wrapper uses its own shutdown channel.
async fn futures_shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
