mod db;
mod state;
mod routes;
mod config;
mod tls;

#[cfg(windows)]
mod service;

use anyhow::Result;
use axum::Router;
use axum_server::Handle;
use tracing::info;

use crate::config::Config;

#[cfg(not(windows))]
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    dispatch_console(&args)
}

#[cfg(windows)]
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("install")     => service::install(),
        Some("uninstall")   => service::uninstall(),
        Some("--service")   => service::run(),
        Some("fingerprint") => print_fingerprint(&args[1..]),
        _ => dispatch_console(&args),
    }
}

fn dispatch_console(args: &[String]) -> Result<()> {
    let cfg_path = parse_config_flag(args);
    let mut cfg = Config::load(cfg_path.as_deref())?;
    cfg.resolve_paths();
    init_logging(&cfg);
    tokio::runtime::Runtime::new()?
        .block_on(run_server(cfg, Handle::new(), console_shutdown()))
}

fn print_fingerprint(args: &[String]) -> Result<()> {
    let cfg_path = parse_config_flag(args);
    let mut cfg = Config::load(cfg_path.as_deref())?;
    cfg.resolve_paths();
    if !cfg.tls.cert_path.exists() {
        anyhow::bail!("No cert at {}. Run the server once to generate it.", cfg.tls.cert_path.display());
    }
    let fp = tls::fingerprint(&cfg.tls.cert_path)?;
    println!("{fp}");
    Ok(())
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

/// Run the HTTP(S) server until `shutdown` resolves (which triggers
/// `handle.graceful_shutdown`).
pub async fn run_server<F>(cfg: Config, handle: Handle, shutdown: F) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if let Some(parent) = cfg.db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let database = db::Database::open(&cfg.db_path)?;
    std::fs::create_dir_all(&cfg.images_dir).ok();
    let mut app_state = state::AppState::new(database).with_images_dir(cfg.images_dir.clone());

    // Load the TLS cert ONCE: used for both the listener and
    // /cert + /fingerprint route responses. `None` when TLS is off.
    let cert_files = if cfg.tls.enabled {
        let files = tls::load_or_generate(&cfg.tls.cert_path, &cfg.tls.key_path)?;
        let pem = std::fs::read_to_string(&files.cert_path)?;
        app_state = app_state.with_tls(pem, files.fingerprint_sha256.clone());
        Some(files)
    } else {
        None
    };

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

    let addr: std::net::SocketAddr = cfg.bind_addr.parse()
        .map_err(|e| anyhow::anyhow!("invalid bind_addr {}: {e}", cfg.bind_addr))?;

    // Spawn the shutdown watcher so the serve future stays owned simply.
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        shutdown.await;
        info!("Shutdown signal received, draining connections");
        handle_clone.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
    });

    if let Some(cert) = cert_files {
        use axum_server::tls_rustls::RustlsConfig;
        let rustls_cfg = RustlsConfig::from_pem_file(&cert.cert_path, &cert.key_path).await?;
        info!("Imaging server listening on https://{addr} (db: {})", cfg.db_path.display());
        axum_server::bind_rustls(addr, rustls_cfg)
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    } else {
        tracing::warn!("TLS disabled -- traffic is PLAINTEXT. Only acceptable for dev.");
        info!("Imaging server listening on http://{addr} (db: {})", cfg.db_path.display());
        axum_server::bind(addr)
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    }

    info!("Imaging server stopped");
    Ok(())
}

/// Ctrl-C wait for console mode.
async fn console_shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
