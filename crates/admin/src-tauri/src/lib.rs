mod db;

use imaging_common::config::*;
use imaging_common::driver::DriverPack;
use imaging_common::security::{CreateKeyRequest, CreateKeyResponse, KeyListEntry};
use imaging_common::sequence::{TaskSequence, TaskStep, StepType, StepConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use uuid::Uuid;

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
    pub enrollment_key: Option<String>,
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
        enrollment_key: request.enrollment_key,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct UsbDevice {
    pub drive_letter: String,
    pub label: String,
    pub size_bytes: u64,
    pub is_removable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaBuildRequest {
    pub mode: String,
    pub image_id: Option<String>,
    pub image_name: Option<String>,
    pub image_os_family: Option<String>,
    pub drive_letter: String,
    pub server_host: Option<String>,
    pub server_port: Option<u16>,
    pub discovery: Option<String>,
    pub capture_unc_path: Option<String>,
    pub capture_account: Option<String>,
    pub enrollment_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaBuildProgress {
    pub step: String,
    pub pct: u8,
    pub message: String,
}

#[tauri::command]
async fn list_usb_devices() -> Result<Vec<UsbDevice>, String> {
    tokio::task::spawn_blocking(|| enumerate_removable_drives())
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

fn enumerate_removable_drives() -> Result<Vec<UsbDevice>, String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile", "-Command",
            "Get-Volume | Where-Object { $_.DriveType -eq 'Removable' -and $_.DriveLetter } | Select-Object DriveLetter, FileSystemLabel, Size | ConvertTo-Json -Compress"
        ])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell error: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();

    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }

    // PowerShell returns a single object (not array) when there's only one result
    let raw: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let items = match &raw {
        serde_json::Value::Array(arr) => arr.clone(),
        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
        _ => return Ok(Vec::new()),
    };

    Ok(items.iter().filter_map(|item| {
        let letter = item.get("DriveLetter")?.as_str()?;
        let label = item.get("FileSystemLabel")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let size = item.get("Size")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Some(UsbDevice {
            drive_letter: format!("{letter}:"),
            label: label.to_string(),
            size_bytes: size,
            is_removable: true,
        })
    }).collect())
}

#[tauri::command]
async fn build_media(request: MediaBuildRequest) -> Result<String, String> {
    // Step 1: Generate config.json
    let config_json = build_config(ConfigBuildRequest {
        mode: request.mode.clone(),
        server_host: request.server_host,
        server_port: request.server_port,
        discovery: request.discovery,
        image_name: request.image_name.clone(),
        image_os_family: request.image_os_family,
        capture_unc_path: request.capture_unc_path,
        capture_account: request.capture_account,
        enrollment_key: request.enrollment_key,
    })?;

    let drive = request.drive_letter.clone();

    tokio::task::spawn_blocking(move || {
        // Step 2: Create directory structure
        let imaging_dir = format!(r"{drive}\imaging");
        std::fs::create_dir_all(&imaging_dir)
            .map_err(|e| format!("Failed to create imaging dir: {e}"))?;

        let logs_dir = format!(r"{drive}\imaging\logs");
        std::fs::create_dir_all(&logs_dir)
            .map_err(|e| format!("Failed to create logs dir: {e}"))?;

        // Step 3: Write config.json
        let config_path = format!(r"{drive}\imaging\config.json");
        std::fs::write(&config_path, &config_json)
            .map_err(|e| format!("Failed to write config.json: {e}"))?;

        tracing::info!("Config written to {config_path}");

        // Note: Actual WinPE boot.wim copy, agent binary copy, and
        // partition formatting require ADK tools and elevated privileges.
        // This writes the config and directory structure; the full
        // build-winpe-media.ps1 script handles the rest.

        Ok(format!("Media build prepared on {drive}. Config written to {config_path}. Run build-winpe-media.ps1 with ADK to complete the boot media."))
    })
    .await
    .map_err(|e| format!("Build task failed: {e}"))?
}

struct AppConfig {
    server_host: String,
    server_port: u16,
    hostname: String,
}

struct DbStore(Arc<Mutex<db::AdminDb>>);

// --- Task Sequences ---

#[tauri::command]
async fn get_sequences(store: tauri::State<'_, DbStore>) -> Result<Vec<TaskSequence>, String> {
    let db = store.0.lock().await;
    db.list_sequences().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_sequence(store: tauri::State<'_, DbStore>, sequence: TaskSequence) -> Result<TaskSequence, String> {
    if let Err(errors) = sequence.validate() {
        return Err(errors.join("; "));
    }

    let mut seq = sequence;
    seq.updated_at = chrono::Utc::now();

    let db = store.0.lock().await;
    db.upsert_sequence(&seq).map_err(|e| e.to_string())?;
    Ok(seq)
}

#[tauri::command]
async fn delete_sequence(store: tauri::State<'_, DbStore>, id: String) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e| format!("Invalid UUID: {e}"))?;
    let db = store.0.lock().await;
    let deleted = db.delete_sequence(&uuid).map_err(|e| e.to_string())?;
    if deleted {
        Ok(())
    } else {
        Err("Sequence not found".into())
    }
}

#[tauri::command]
fn new_sequence() -> TaskSequence {
    let now = chrono::Utc::now();
    TaskSequence {
        id: Uuid::new_v4(),
        name: "New Task Sequence".into(),
        description: String::new(),
        steps: vec![
            TaskStep {
                id: Uuid::new_v4(),
                step_type: StepType::Partition,
                name: "Partition Disk".into(),
                enabled: true,
                order: 0,
                config: StepConfig::Partition {
                    esp_size_mb: 260,
                    msr_size_mb: 16,
                },
            },
            TaskStep {
                id: Uuid::new_v4(),
                step_type: StepType::ApplyImage,
                name: "Apply Image".into(),
                enabled: true,
                order: 1,
                config: StepConfig::ApplyImage { image_index: 1 },
            },
            TaskStep {
                id: Uuid::new_v4(),
                step_type: StepType::Reboot,
                name: "Reboot".into(),
                enabled: true,
                order: 2,
                config: StepConfig::Reboot { delay_seconds: 0 },
            },
        ],
        created_at: now,
        updated_at: now,
    }
}

#[tauri::command]
fn new_step(step_type: String) -> Result<TaskStep, String> {
    let st: StepType = serde_json::from_value(serde_json::Value::String(step_type.clone()))
        .map_err(|_| format!("Invalid step type: {step_type}"))?;

    let config = match st {
        StepType::Partition => StepConfig::Partition {
            esp_size_mb: 260,
            msr_size_mb: 16,
        },
        StepType::ApplyImage => StepConfig::ApplyImage { image_index: 1 },
        StepType::InjectDrivers => StepConfig::InjectDrivers {
            driver_pack_id: None,
            driver_path: None,
        },
        StepType::RunScript => StepConfig::RunScript {
            script_path: String::new(),
            arguments: Vec::new(),
            run_elevated: false,
        },
        StepType::DomainJoin => StepConfig::DomainJoin {
            domain_name: String::new(),
            ou_path: None,
            account: String::new(),
        },
        StepType::GenerateUnattend => StepConfig::GenerateUnattend {
            computer_name: None,
            locale: None,
            timezone: None,
            product_key: None,
        },
        StepType::Reboot => StepConfig::Reboot { delay_seconds: 0 },
    };

    Ok(TaskStep {
        id: Uuid::new_v4(),
        step_type: st,
        name: format!("{st}"),
        enabled: true,
        order: 0,
        config,
    })
}

// --- Driver Packs ---

#[tauri::command]
async fn get_driver_packs(store: tauri::State<'_, DbStore>) -> Result<Vec<DriverPack>, String> {
    let db = store.0.lock().await;
    db.list_driver_packs().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_driver_pack(store: tauri::State<'_, DbStore>, pack: DriverPack) -> Result<DriverPack, String> {
    let mut p = pack;
    if p.created_at == chrono::DateTime::<chrono::Utc>::default() {
        p.created_at = chrono::Utc::now();
    }
    let db = store.0.lock().await;
    db.upsert_driver_pack(&p).map_err(|e| e.to_string())?;
    Ok(p)
}

#[tauri::command]
async fn delete_driver_pack(store: tauri::State<'_, DbStore>, id: String) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e| format!("Invalid UUID: {e}"))?;
    let db = store.0.lock().await;
    let deleted = db.delete_driver_pack(&uuid).map_err(|e| e.to_string())?;
    if deleted {
        Ok(())
    } else {
        Err("Driver pack not found".into())
    }
}

// --- Unattend Preview ---

#[tauri::command]
fn preview_unattend(config: UnattendRequest) -> Result<String, String> {
    let uc = imaging_engine::unattend::UnattendConfig {
        computer_name: config.computer_name,
        locale: config.locale.unwrap_or_else(|| "en-US".into()),
        timezone: config.timezone.unwrap_or_else(|| "Eastern Standard Time".into()),
        product_key: config.product_key,
        admin_password: None,
        domain_join: config.domain_name.map(|dn| {
            imaging_engine::unattend::DomainJoinConfig {
                domain_name: dn,
                ou_path: config.ou_path,
                account: config.domain_account.unwrap_or_default(),
                password: String::new(),
            }
        }),
    };
    imaging_engine::unattend::generate_unattend_xml(&uc).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnattendRequest {
    pub computer_name: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub product_key: Option<String>,
    pub domain_name: Option<String>,
    pub ou_path: Option<String>,
    pub domain_account: Option<String>,
}

// --- Deploy Monitoring ---

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployJobEntry {
    pub id: String,
    pub machine_name: String,
    pub image_id: String,
    pub status: String,
    pub progress_pct: u8,
    pub started_at: String,
    pub started_by: String,
    pub message: String,
}

#[tauri::command]
async fn get_deploy_jobs(state: tauri::State<'_, AppConfig>) -> Result<Vec<DeployJobEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let base = format!("http://{}:{}", state.server_host, state.server_port);

    let auth_resp = client
        .post(format!("{base}/api/auth"))
        .json(&serde_json::json!({ "machine_name": state.hostname }))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json::<imaging_common::api::AuthResponse>()
        .await.map_err(|e| e.to_string())?;

    let jobs: Vec<DeployJobEntry> = client
        .get(format!("{base}/api/deploy/jobs"))
        .bearer_auth(&auth_resp.session_token)
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    Ok(jobs)
}

// --- Audit Log ---

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntryJs {
    pub id: String,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub detail: Option<String>,
    pub level: String,
}

#[tauri::command]
async fn get_audit_log(state: tauri::State<'_, AppConfig>, search: Option<String>, level: Option<String>) -> Result<Vec<AuditEntryJs>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let base = format!("http://{}:{}", state.server_host, state.server_port);

    let auth_resp = client
        .post(format!("{base}/api/auth"))
        .json(&serde_json::json!({ "machine_name": state.hostname }))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json::<imaging_common::api::AuthResponse>()
        .await.map_err(|e| e.to_string())?;

    let mut url = format!("{base}/api/audit?limit=200");
    if let Some(ref s) = search {
        url.push_str(&format!("&search={}", urlencoding(s)));
    }
    if let Some(ref l) = level {
        url.push_str(&format!("&level={l}"));
    }

    let entries: Vec<AuditEntryJs> = client
        .get(&url)
        .bearer_auth(&auth_resp.session_token)
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    Ok(entries)
}

#[tauri::command]
async fn export_audit_csv(state: tauri::State<'_, AppConfig>) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let base = format!("http://{}:{}", state.server_host, state.server_port);

    let auth_resp = client
        .post(format!("{base}/api/auth"))
        .json(&serde_json::json!({ "machine_name": state.hostname }))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json::<imaging_common::api::AuthResponse>()
        .await.map_err(|e| e.to_string())?;

    let csv = client
        .get(format!("{base}/api/audit/export?limit=1000"))
        .bearer_auth(&auth_resp.session_token)
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;

    Ok(csv)
}

// --- Enrollment Keys ---

#[tauri::command]
async fn get_enrollment_keys(state: tauri::State<'_, AppConfig>) -> Result<Vec<KeyListEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let base = format!("http://{}:{}", state.server_host, state.server_port);

    let keys: Vec<KeyListEntry> = client
        .get(format!("{base}/api/keys"))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    Ok(keys)
}

#[tauri::command]
async fn create_enrollment_key(state: tauri::State<'_, AppConfig>, label: String, expires_in_days: u32) -> Result<CreateKeyResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let base = format!("http://{}:{}", state.server_host, state.server_port);

    let resp: CreateKeyResponse = client
        .post(format!("{base}/api/keys"))
        .json(&CreateKeyRequest { label, expires_in_days })
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    Ok(resp)
}

#[tauri::command]
async fn revoke_enrollment_key(state: tauri::State<'_, AppConfig>, id: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let base = format!("http://{}:{}", state.server_host, state.server_port);

    client
        .post(format!("{base}/api/keys/{id}/revoke"))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?;

    Ok(())
}

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| match c {
        ' ' => "%20".to_string(),
        '&' => "%26".to_string(),
        '=' => "%3D".to_string(),
        _ => c.to_string(),
    }).collect()
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

            let app_data = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&app_data).ok();
            let db_path = app_data.join("deploystation.db");
            let admin_db = db::AdminDb::open(&db_path)
                .expect("Failed to open admin database");
            app.manage(DbStore(Arc::new(Mutex::new(admin_db))));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_mode,
            get_version,
            get_dashboard_stats,
            get_images,
            build_config,
            validate_config,
            list_usb_devices,
            build_media,
            get_sequences,
            save_sequence,
            delete_sequence,
            new_sequence,
            new_step,
            get_driver_packs,
            save_driver_pack,
            delete_driver_pack,
            preview_unattend,
            get_deploy_jobs,
            get_audit_log,
            export_audit_csv,
            get_enrollment_keys,
            create_enrollment_key,
            revoke_enrollment_key,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
