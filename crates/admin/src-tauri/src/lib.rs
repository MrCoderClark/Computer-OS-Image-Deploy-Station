use imaging_common::config::*;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigBuildRequest {
    pub mode: String,
    pub server_host: Option<String>,
    pub server_port: Option<u16>,
    pub discovery: Option<String>,
    pub image_name: Option<String>,
    pub image_os_family: Option<String>,
    pub capture_unc_path: Option<String>,
    pub capture_account: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub active_deploys: u32,
    pub images_count: u32,
    pub media_built: u32,
    pub captures_pending: u32,
    pub server_connected: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageEntry {
    pub id: String,
    pub name: String,
    pub os_family: String,
    pub captured_from: Option<String>,
    pub build_date: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
}

#[tauri::command]
fn get_mode() -> String {
    "No configuration loaded".to_string()
}

#[tauri::command]
fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn get_dashboard_stats(state: tauri::State<'_, AppConfig>) -> Result<DashboardStats, String> {
    let images = fetch_images_from_server(&state).await;
    let count = images.as_ref().map(|v| v.len() as u32).unwrap_or(0);

    Ok(DashboardStats {
        active_deploys: 0,
        images_count: count,
        media_built: 0,
        captures_pending: 0,
        server_connected: images.is_ok(),
    })
}

#[tauri::command]
async fn get_images(state: tauri::State<'_, AppConfig>) -> Result<Vec<ImageEntry>, String> {
    let images = fetch_images_from_server(&state).await.map_err(|e| e.to_string())?;
    Ok(images)
}

async fn fetch_images_from_server(config: &AppConfig) -> anyhow::Result<Vec<ImageEntry>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let base = format!("http://{}:{}", config.server_host, config.server_port);

    let auth_resp = client
        .post(format!("{base}/api/auth"))
        .json(&serde_json::json!({
            "machine_name": config.hostname,
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<imaging_common::api::AuthResponse>()
        .await?;

    let list_resp = client
        .get(format!("{base}/api/images"))
        .bearer_auth(&auth_resp.session_token)
        .send()
        .await?
        .error_for_status()?
        .json::<imaging_common::api::ImageListResponse>()
        .await?;

    Ok(list_resp
        .images
        .into_iter()
        .map(|img| ImageEntry {
            id: img.id.to_string(),
            name: img.name,
            os_family: format!("{}", img.os_family),
            captured_from: img.captured_from,
            build_date: img.build_date,
            size_bytes: img.size_bytes,
            sha256: img.sha256,
        })
        .collect())
}

#[tauri::command]
fn build_config(request: ConfigBuildRequest) -> Result<String, String> {
    let mode: OperatingMode = serde_json::from_value(
        serde_json::Value::String(request.mode.clone()),
    )
    .map_err(|_| format!("Invalid mode: {}", request.mode))?;

    let server = if matches!(mode, OperatingMode::Online | OperatingMode::Hybrid) {
        let discovery_str = request.discovery.as_deref().unwrap_or("fixed");
        let discovery: DiscoveryMethod = serde_json::from_value(
            serde_json::Value::String(discovery_str.to_string()),
        )
        .map_err(|_| format!("Invalid discovery method: {discovery_str}"))?;

        Some(ServerConfig {
            discovery,
            host: request.server_host.clone(),
            port: request.server_port.unwrap_or(7443),
        })
    } else {
        None
    };

    let image = if matches!(mode, OperatingMode::Offline | OperatingMode::Hybrid) {
        let os_str = request.image_os_family.as_deref().unwrap_or("win11");
        let os_family: OsFamily = serde_json::from_value(
            serde_json::Value::String(os_str.to_string()),
        )
        .map_err(|_| format!("Invalid os_family: {os_str}"))?;

        Some(ImageMeta {
            name: request.image_name.unwrap_or_else(|| "deploy".into()),
            os_family,
            captured_from: None,
            build_date: None,
            size_bytes: None,
            sha256: None,
            wim_path: None,
        })
    } else {
        None
    };

    let capture_target = request.capture_unc_path.map(|unc| CaptureTarget {
        target_type: "share".into(),
        unc_path: unc,
        account: request.capture_account.unwrap_or_default(),
    });

    let config = SuiteConfig {
        schema_version: CURRENT_SCHEMA_VERSION,
        mode,
        server,
        capture_target,
        image,
        built_by: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "admin".into()),
        built_at: chrono::Utc::now(),
    };

    if let Err(errors) = config.validate() {
        return Err(errors.join("; "));
    }

    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

#[tauri::command]
fn validate_config(json_str: String) -> Result<String, String> {
    let config: SuiteConfig =
        serde_json::from_str(&json_str).map_err(|e| format!("Invalid JSON: {e}"))?;

    match config.validate() {
        Ok(()) => Ok("Configuration is valid".into()),
        Err(errors) => Err(errors.join("; ")),
    }
}

struct AppConfig {
    server_host: String,
    server_port: u16,
    hostname: String,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tracing_subscriber::fmt()
                .with_env_filter("imaging_admin=debug")
                .init();

            tracing::info!("DeployStation Admin v{}", app.package_info().version);

            let hostname = hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "admin".into());

            app.manage(AppConfig {
                server_host: "127.0.0.1".into(),
                server_port: 7443,
                hostname,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_mode,
            get_version,
            get_dashboard_stats,
            get_images,
            build_config,
            validate_config,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
