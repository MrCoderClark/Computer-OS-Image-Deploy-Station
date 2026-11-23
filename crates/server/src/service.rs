//! Windows Service integration for imaging-server.
//!
//! The same binary runs either as a console app (default) or as a Windows
//! Service (when invoked with `--service`, which the service control manager
//! supplies as the configured BinPath argument).
//!
//! Subcommands `install` / `uninstall` register and remove the service.

#![cfg(windows)]

use anyhow::{anyhow, Result};
use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;

use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl,
        ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher, service_manager::{ServiceManager, ServiceManagerAccess},
};

pub const SERVICE_NAME: &str = "DeployStationServer";
pub const SERVICE_DISPLAY: &str = "DeployStation Imaging Server";
pub const SERVICE_DESCRIPTION: &str =
    "DeployStation imaging server: image catalog, deployment orchestration, audit.";

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

/// Called by the SCM-dispatched entry point. Blocks running the async server
/// on a fresh tokio runtime until the SCM signals stop.
pub fn run() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .map_err(|e| anyhow!("service_dispatcher::start: {e}"))?;
    Ok(())
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = service_main_inner() {
        tracing::error!("service_main failed: {e}");
    }
}

fn service_main_inner() -> Result<()> {
    // Set up logging first so anything that follows is captured. Load config
    // just to find log_dir / log_filter; the full config load happens again
    // inside run_server (cheap, and keeps the shared config path).
    let mut cfg = crate::config::Config::load(None).unwrap_or_default();
    cfg.resolve_paths();
    let filter = cfg.log_filter.clone();
    if let Some(dir) = cfg.log_dir.as_ref() {
        let _ = std::fs::create_dir_all(dir);
        let file = tracing_appender::rolling::daily(dir, "imaging-server.log");
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    } else {
        // No log_dir -> file discarded; service has no stdout. Still init so
        // any later writer swap is possible.
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    }

    // Channel the SCM uses to tell the server to stop.
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .map_err(|e| anyhow!("register control handler: {e}"))?;

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    // Spin the server on a tokio runtime and wait for the SCM stop signal.
    let runtime = tokio::runtime::Runtime::new()?;
    let (server_done_tx, server_done_rx) = mpsc::channel::<Result<()>>();

    std::thread::spawn(move || {
        let res = runtime.block_on(async {
            let mut cfg = crate::config::Config::load(None)
                .unwrap_or_else(|e| {
                    tracing::warn!("Config load failed: {e}; using defaults");
                    crate::config::Config::default()
                });
            cfg.resolve_paths();
            // init_logging is a crate-private helper in main; we skip it in
            // service mode -- tracing-subscriber refuses a second init anyway.
            let handle = axum_server::Handle::new();
            crate::run_server(cfg, handle, async move {
                tokio::task::spawn_blocking(move || { let _ = shutdown_rx.recv(); })
                    .await
                    .ok();
            }).await
        });
        let _ = server_done_tx.send(res);
    });

    // Wait until the server thread finishes (either the SCM signaled stop
    // and the server unwound, or it crashed).
    let server_result = server_done_rx.recv().unwrap_or_else(|e| Err(anyhow!("server channel closed: {e}")));

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: match &server_result {
            Ok(_)  => ServiceExitCode::Win32(0),
            Err(_) => ServiceExitCode::Win32(1),
        },
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    server_result
}

pub fn install() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
        .map_err(|e| anyhow!("open SCM (need admin): {e}"))?;

    let exe = std::env::current_exe()
        .map_err(|e| anyhow!("current_exe: {e}"))?;

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY),
        service_type: SERVICE_TYPE,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("--service")],
        dependencies: vec![],
        account_name: None,  // LocalSystem
        account_password: None,
    };

    let service = manager.create_service(&info, ServiceAccess::CHANGE_CONFIG)
        .map_err(|e| anyhow!("create_service: {e}"))?;
    service.set_description(SERVICE_DESCRIPTION)
        .map_err(|e| anyhow!("set_description: {e}"))?;

    println!("Installed service '{SERVICE_NAME}'.");
    println!("Start with: sc start {SERVICE_NAME}");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| anyhow!("open SCM (need admin): {e}"))?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS)
        .map_err(|e| anyhow!("open_service: {e}"))?;

    // Best-effort stop before delete.
    if let Ok(status) = service.query_status() {
        if status.current_state != ServiceState::Stopped {
            let _ = service.stop();
            // SCM queues the delete until the service exits; a short wait
            // keeps the user's terminal output ordered.
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    service.delete().map_err(|e| anyhow!("delete: {e}"))?;
    println!("Uninstalled service '{SERVICE_NAME}'.");
    Ok(())
}
