mod db;

use imaging_common::config::*;
use imaging_common::driver::DriverPack;
use imaging_common::security::{CreateKeyRequest, CreateKeyResponse, KeyListEntry};
use imaging_common::sequence::{TaskSequence, TaskStep, StepType, StepConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, Manager};
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
pub struct UsbDisk {
    pub disk_number: u32,
    pub friendly_name: String,
    pub size_bytes: u64,
    pub partition_count: u32,
    pub is_usb: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaBuildRequest {
    pub mode: String,
    pub disk_number: u32,
    pub image_id: Option<String>,
    pub image_name: Option<String>,
    pub image_os_family: Option<String>,
    pub wim_source_path: Option<String>,
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
async fn list_usb_disks() -> Result<Vec<UsbDisk>, String> {
    tokio::task::spawn_blocking(|| enumerate_usb_disks())
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

fn enumerate_usb_disks() -> Result<Vec<UsbDisk>, String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command",
            "Get-Disk | Where-Object { $_.BusType -eq 'USB' } | Select-Object Number, FriendlyName, Size, NumberOfPartitions | ConvertTo-Json -Compress"
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

    let raw: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let items = match &raw {
        serde_json::Value::Array(arr) => arr.clone(),
        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
        _ => return Ok(Vec::new()),
    };

    Ok(items.iter().filter_map(|item| {
        let number = item.get("Number")?.as_u64()? as u32;
        let name = item.get("FriendlyName")
            .and_then(|v| v.as_str())
            .unwrap_or("USB Disk");
        let size = item.get("Size")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let partitions = item.get("NumberOfPartitions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        Some(UsbDisk {
            disk_number: number,
            friendly_name: name.to_string(),
            size_bytes: size,
            partition_count: partitions,
            is_usb: true,
        })
    }).collect())
}

fn run_ps(script: &str) -> Result<String, String> {
    // -ExecutionPolicy Bypass: our injector script lives in tools/ and is
    // unsigned; the machine-wide policy may be Restricted/AllSigned, which
    // would block `& '...\winpe-inject-deps.ps1'` without this flag.
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("{stderr}\n{stdout}").trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn find_adk_path() -> Result<String, String> {
    let default = r"C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit";
    let deploy_tools = format!(r"{default}\Deployment Tools");
    let winpe_addon = format!(r"{default}\Windows Preinstallation Environment");

    if !std::path::Path::new(&deploy_tools).exists() {
        return Err("Windows ADK Deployment Tools not found. Install the Windows 11 ADK (24H2+).".into());
    }
    if !std::path::Path::new(&winpe_addon).exists() {
        return Err("WinPE add-on not found. Install the Windows PE add-on for the Windows ADK.".into());
    }

    Ok(default.to_string())
}

fn find_agent_binary() -> Result<std::path::PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(ref dir) = exe_dir {
        candidates.push(dir.join("imaging-agent.exe"));
    }

    candidates.push(std::path::PathBuf::from(r"target\release\imaging-agent.exe"));
    candidates.push(std::path::PathBuf::from(r"target\debug\imaging-agent.exe"));
    candidates.push(std::path::PathBuf::from(r"..\..\..\target\release\imaging-agent.exe"));
    candidates.push(std::path::PathBuf::from(r"..\..\..\target\debug\imaging-agent.exe"));
    candidates.push(std::path::PathBuf::from(r"imaging-agent.exe"));

    for candidate in &candidates {
        if candidate.exists() {
            let resolved = candidate.canonicalize().unwrap_or_else(|_| candidate.clone());
            let resolved = strip_unc_prefix(resolved);
            tracing::info!("Found agent binary at {}", resolved.display());
            return Ok(resolved);
        }
    }

    Err("imaging-agent.exe not found. Build it first: cargo build --release -p imaging-agent".into())
}

fn find_tools_dir() -> Result<std::path::PathBuf, String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        let mut cur = dir;
        for _ in 0..6 {
            candidates.push(cur.join("tools"));
            if !cur.pop() { break; }
        }
    }
    candidates.push(std::path::PathBuf::from("tools"));
    candidates.push(std::path::PathBuf::from(r"..\..\..\tools"));

    for c in &candidates {
        if c.join("winpe-inject-deps.ps1").exists() {
            let resolved = c.canonicalize().unwrap_or_else(|_| c.clone());
            let resolved = strip_unc_prefix(resolved);
            tracing::info!("tools/ dir: {}", resolved.display());
            return Ok(resolved);
        }
    }
    Err("tools/ directory with winpe-inject-deps.ps1 not found.".into())
}

/// Strip the Windows `\\?\` extended-length prefix. Rust's `canonicalize()`
/// returns paths with this prefix, which PowerShell 5.1 cmdlets like
/// `Join-Path` cannot parse ("drive is null"). Standard tools (diskpart,
/// Copy-Item, DISM) are fine with it, but anything PSDrive-based is not.
fn strip_unc_prefix(p: std::path::PathBuf) -> std::path::PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        // `\\?\UNC\server\share\...` must become `\\server\share\...`
        if let Some(unc) = rest.strip_prefix("UNC\\") {
            return std::path::PathBuf::from(format!(r"\\{unc}"));
        }
        return std::path::PathBuf::from(rest);
    }
    p
}

fn is_running_elevated() -> bool {
    run_ps("([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)")
        .map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[tauri::command]
async fn build_media(request: MediaBuildRequest, app: tauri::AppHandle) -> Result<String, String> {
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

    let disk_number = request.disk_number;
    let mode = request.mode.clone();
    let wim_source = request.wim_source_path.clone();

    tokio::task::spawn_blocking(move || {
        let emit = |step: &str, pct: u8, msg: &str| {
            let _ = app.emit("build-progress", MediaBuildProgress {
                step: step.into(),
                pct,
                message: msg.into(),
            });
        };

        // --- Validate prerequisites ---
        emit("validate", 0, "Checking prerequisites...");

        if !is_running_elevated() {
            return Err("Administrator privileges required. Restart DeployStation as Administrator.".into());
        }

        let adk_path = find_adk_path()?;
        let deploy_tools = format!(r"{adk_path}\Deployment Tools");
        let winpe_addon = format!(r"{adk_path}\Windows Preinstallation Environment");

        let agent_path = find_agent_binary()?;
        tracing::info!("Agent binary: {}", agent_path.display());

        let needs_wim = mode == "offline" || mode == "hybrid";
        if needs_wim {
            if let Some(ref wim) = wim_source {
                if !std::path::Path::new(wim).exists() {
                    return Err(format!("WIM file not found: {wim}"));
                }
            } else {
                return Err("WIM file path required for offline/hybrid mode.".into());
            }
        }

        // --- Clean disk ---
        emit("disk", 5, &format!("Cleaning disk {disk_number}..."));
        tracing::info!("Cleaning disk {disk_number}");

        // Ensure automount is enabled via script file (pipe to diskpart is unreliable)
        let dp_enable = format!(r"{}\automount.txt", std::env::temp_dir().to_string_lossy().trim_end_matches('\\'));
        std::fs::write(&dp_enable, "automount enable\nexit\n").ok();
        run_ps(&format!("diskpart /s '{dp_enable}'")).ok();
        std::fs::remove_file(&dp_enable).ok();

        // Stop ShellHWDetection to prevent "format disk" popups during partitioning
        run_ps("Stop-Service ShellHWDetection -Force -ErrorAction SilentlyContinue").ok();

        if needs_wim {
            emit("disk", 10, "Creating boot and data partitions...");
            let dp_script = format!(r"{}\deploystation_dp.txt", std::env::temp_dir().to_string_lossy().trim_end_matches('\\'));
            std::fs::write(&dp_script, format!(
                "select disk {disk_number}\nclean\nconvert mbr\ncreate partition primary size=2048\nformat fs=fat32 quick label=DEPLOY_BOOT\nactive\nassign\ncreate partition primary\nformat fs=ntfs quick label=DEPLOY_DATA\nassign\nexit\n"
            )).map_err(|e| format!("Failed to write diskpart script: {e}"))?;
            run_ps(&format!("diskpart /s '{dp_script}'"))
                .map_err(|e| format!("Failed to partition disk: {e}"))?;
            std::fs::remove_file(&dp_script).ok();
        } else {
            emit("disk", 10, "Creating FAT32 boot partition...");
            let dp_script = format!(r"{}\deploystation_dp.txt", std::env::temp_dir().to_string_lossy().trim_end_matches('\\'));
            std::fs::write(&dp_script, format!(
                "select disk {disk_number}\nclean\nconvert mbr\ncreate partition primary\nformat fs=fat32 quick label=DEPLOY_USB\nactive\nassign\nexit\n"
            )).map_err(|e| format!("Failed to write diskpart script: {e}"))?;
            run_ps(&format!("diskpart /s '{dp_script}'"))
                .map_err(|e| format!("Failed to partition disk: {e}"))?;
            std::fs::remove_file(&dp_script).ok();
        }

        // Restart ShellHWDetection
        run_ps("Start-Service ShellHWDetection -ErrorAction SilentlyContinue").ok();

        // Get drive letter — simple approach, no fancy checks
        emit("disk", 15, "Identifying partitions...");
        std::thread::sleep(std::time::Duration::from_secs(3));

        let boot_letter = run_ps(&format!(
            "$dl = (Get-Partition -DiskNumber {disk_number} | Where-Object {{ $_.Size -gt 0 }} | Sort-Object PartitionNumber | Select-Object -First 1).DriveLetter; \
             if ($dl) {{ $dl }} else {{ '' }}"
        )).map_err(|e| format!("Failed to get boot drive letter: {e}"))?;
        let boot_letter = boot_letter.trim().to_string();

        if boot_letter.is_empty() {
            return Err("No drive letter on boot partition. Run 'echo automount enable | diskpart' in an admin prompt, then retry.".into());
        }

        let boot_drive = format!("{boot_letter}:");
        tracing::info!("Boot partition: {boot_drive}");

        if needs_wim {
            let data_letter = run_ps(&format!(
                "$dl = (Get-Partition -DiskNumber {disk_number} | Where-Object {{ $_.Size -gt 0 }} | Sort-Object PartitionNumber | Select-Object -Last 1).DriveLetter; \
                 if ($dl) {{ $dl }} else {{ '' }}"
            )).map_err(|e| format!("Failed to get data drive letter: {e}"))?;
            let data_letter = data_letter.trim().to_string();
            if data_letter.is_empty() {
                return Err("No drive letter on data partition.".into());
            }

            emit("wim", 20, "Copying WIM to USB (this may take several minutes)...");
            let wim = wim_source.as_ref().unwrap();
            let dest_dir = format!(r"{data_letter}:\images");
            std::fs::create_dir_all(&dest_dir)
                .map_err(|e| format!("Failed to create images dir: {e}"))?;
            let dest_path = format!(r"{dest_dir}\deploy.wim");
            std::fs::copy(wim, &dest_path)
                .map_err(|e| format!("Failed to copy WIM: {e}"))?;
            tracing::info!("WIM copied to {dest_path}");
        }

        // --- Build WinPE ---
        emit("winpe", 35, "Creating WinPE working directory...");

        let temp = std::env::temp_dir();
        let temp_str = temp.to_string_lossy();
        let temp_str = temp_str.trim_end_matches('\\');
        let work_dir = format!(r"{temp_str}\DeployStation-WinPE-{}", std::process::id());

        let copype_script = format!(
            "$env:WinPERoot = '{winpe_addon}'; \
             $env:DISMRoot = '{deploy_tools}\\amd64\\DISM'; \
             $env:OSCDImgRoot = '{deploy_tools}\\amd64\\Oscdimg'; \
             $env:BCDBootRoot = '{deploy_tools}\\amd64\\BCDBoot'; \
             $env:ImagingRoot = '{deploy_tools}\\amd64'; \
             $env:Path = '{winpe_addon};{deploy_tools}\\amd64\\DISM;{deploy_tools}\\amd64;{deploy_tools};' + $env:Path; \
             & cmd.exe /c \"`\"{winpe_addon}\\copype.cmd`\" amd64 `\"{work_dir}`\"\" 2>&1 | Out-Null; \
             if (-not (Test-Path '{work_dir}\\media')) {{ throw 'copype failed' }}"
        );
        run_ps(&copype_script).map_err(|e| format!("Failed to create WinPE directory: {e}"))?;

        // --- Mount boot.wim and inject agent ---
        emit("winpe", 45, "Mounting boot.wim and injecting agent...");

        let boot_wim = format!(r"{work_dir}\media\sources\boot.wim");
        let mount_dir = format!(r"{work_dir}\mount");
        std::fs::create_dir_all(&mount_dir).ok();

        run_ps(&format!(
            "Mount-WindowsImage -ImagePath '{boot_wim}' -Index 1 -Path '{mount_dir}' -ErrorAction Stop"
        )).map_err(|e| format!("Failed to mount boot.wim: {e}"))?;

        // Delegate agent copy + full DLL closure walk + Mesa3D llvmpipe install to
        // the shared PowerShell helper. WinPE lacks a GPU driver, so the agent
        // (eframe/glow → OpenGL) needs Mesa's software renderer, plus every
        // transitive static import the agent brings in.
        //
        // The tools/ directory is next to the agent binary's source tree. In
        // `cargo tauri dev` the CWD is the workspace root, so relative paths
        // resolve. For a packaged release build these resources will need to be
        // bundled via tauri.conf.json — TODO when we ship the installer.
        emit("winpe", 50, "Injecting agent, DLL closure, and Mesa3D...");

        let tools_dir = find_tools_dir()?;
        let inject_script = tools_dir.join("winpe-inject-deps.ps1");
        let mesa_dir = tools_dir.join("mesa");
        if !inject_script.exists() {
            return Err(format!("Injector script missing: {}", inject_script.display()));
        }
        if !mesa_dir.join("opengl32.dll").exists() {
            return Err(format!(
                "Mesa3D DLLs missing at {}. Download the x64 release-msvc build \
                 from https://github.com/pal1000/mesa-dist-win/releases and copy \
                 x64\\opengl32.dll + x64\\libgallium_wgl.dll into that folder.",
                mesa_dir.display()
            ));
        }
        run_ps(&format!(
            "& '{}' -MountDir '{mount_dir}' -AgentPath '{}' -MesaDir '{}'",
            inject_script.to_string_lossy(),
            agent_path.to_string_lossy(),
            mesa_dir.to_string_lossy()
        )).map_err(|e| format!("Agent/DLL injection failed: {e}"))?;
        tracing::info!("Agent + DLL closure + Mesa3D injected into boot.wim");

        // Write startnet.cmd with diagnostics on failure
        emit("winpe", 55, "Configuring WinPE startup...");
        let startnet = format!(r"{mount_dir}\Windows\System32\startnet.cmd");
        let startnet_content = "\
@echo off\r\n\
wpeinit\r\n\
echo.\r\n\
echo ============================================\r\n\
echo   DeployStation - Windows Imaging Agent\r\n\
echo ============================================\r\n\
echo.\r\n\
REM Force Mesa3D to use its llvmpipe software renderer directly; skips the\r\n\
REM failed ZINK/Vulkan probe that prints a harmless error at startup.\r\n\
set GALLIUM_DRIVER=llvmpipe\r\n\
echo SystemRoot = %SystemRoot%\r\n\
echo Starting agent...\r\n\
imaging-agent.exe\r\n\
set AGENT_EXIT=%errorlevel%\r\n\
if %AGENT_EXIT% NEQ 0 (\r\n\
    echo.\r\n\
    echo !! Agent exited with code %AGENT_EXIT%\r\n\
    echo.\r\n\
    echo --- Diagnostics ---\r\n\
    echo Checking DLLs in %SystemRoot%\\System32:\r\n\
    for %%D in (opengl32.dll libgallium_wgl.dll dwmapi.dll imm32.dll uxtheme.dll bcryptprimitives.dll crypt32.dll secur32.dll shlwapi.dll shell32.dll ole32.dll user32.dll gdi32.dll gdi32full.dll kernel32.dll ntdll.dll vcruntime140.dll vcruntime140_1.dll msvcp140.dll api-ms-win-core-synch-l1-2-0.dll) do (\r\n\
        if exist %SystemRoot%\\System32\\%%D (echo   %%D ... OK) else (echo   %%D ... MISSING)\r\n\
    )\r\n\
    echo.\r\n\
    echo Agent binary:\r\n\
    dir %SystemRoot%\\System32\\imaging-agent.exe\r\n\
    echo.\r\n\
    echo All DLLs in System32 starting with o:\r\n\
    dir %SystemRoot%\\System32\\o*.dll\r\n\
)\r\n\
echo.\r\n\
echo Press any key to open command prompt...\r\n\
pause >nul\r\n\
cmd.exe\r\n";
        std::fs::write(&startnet, startnet_content)
            .map_err(|e| format!("Failed to write startnet.cmd: {e}"))?;

        // Unmount and save
        emit("winpe", 60, "Saving boot.wim (this takes a moment)...");
        run_ps(&format!(
            "Dismount-WindowsImage -Path '{mount_dir}' -Save -ErrorAction Stop"
        )).map_err(|e| format!("Failed to unmount boot.wim: {e}"))?;

        // --- Copy boot files to USB ---
        emit("copy", 70, "Copying WinPE boot files to USB...");

        let media_dir = format!(r"{work_dir}\media");

        run_ps(&format!(
            "& robocopy '{media_dir}' '{boot_drive}\\' /E /R:3 /W:2 /NFL /NDL /NJH /NJS /NP; if ($LASTEXITCODE -ge 8) {{ throw \"robocopy failed with exit code $LASTEXITCODE\" }}; exit 0"
        )).map_err(|e| format!("Failed to copy boot files: {e}"))?;

        // --- Write config ---
        emit("config", 85, "Writing configuration...");

        let imaging_dir = format!(r"{boot_drive}\imaging");
        std::fs::create_dir_all(&imaging_dir)
            .map_err(|e| format!("Failed to create imaging dir: {e}"))?;
        std::fs::create_dir_all(format!(r"{imaging_dir}\logs"))
            .map_err(|e| format!("Failed to create logs dir: {e}"))?;

        let config_path = format!(r"{imaging_dir}\config.json");
        std::fs::write(&config_path, &config_json)
            .map_err(|e| format!("Failed to write config.json: {e}"))?;
        tracing::info!("Config written to {config_path}");

        // --- Cleanup ---
        emit("cleanup", 95, "Cleaning up temporary files...");
        std::fs::remove_dir_all(&work_dir).ok();

        emit("done", 100, "Build complete!");
        Ok(format!("Bootable USB created on {boot_drive}"))
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

#[tauri::command]
async fn browse_wim_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // blocking_pick_file() parks the thread until the user closes the native
    // dialog — run it off the tokio runtime so the webview and other commands
    // keep responding while the picker is open.
    let result = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("WIM / ESD Images", &["wim", "esd"])
            .add_filter("All Files", &["*"])
            .set_title("Select WIM image file")
            .blocking_pick_file()
    })
    .await
    .map_err(|e| format!("dialog task failed: {e}"))?;

    Ok(result.map(|p| p.to_string()))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            list_usb_disks,
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
            browse_wim_file,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
