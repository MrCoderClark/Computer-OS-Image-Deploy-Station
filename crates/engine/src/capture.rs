use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

use imaging_common::config::CaptureTarget;
use imaging_common::types::CaptureManifest;

pub fn probe_share(target: &CaptureTarget) -> Result<()> {
    info!("Probing capture share: {}", target.unc_path);

    let unc = Path::new(&target.unc_path);
    if unc.exists() {
        info!("Share {} is already mapped and reachable", target.unc_path);
        return Ok(());
    }

    info!("Share not mapped — attempting net use with service account");
    let output = Command::new("net")
        .args(["use", &target.unc_path, &format!("/user:{}", target.account)])
        .output()
        .context("failed to execute 'net use'")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        bail!(
            "Cannot reach capture share {}: {}{}",
            target.unc_path,
            stdout.trim(),
            if stderr.is_empty() { String::new() } else { format!(" — {}", stderr.trim()) }
        );
    }

    if !Path::new(&target.unc_path).exists() {
        bail!(
            "net use succeeded but share {} is still not accessible",
            target.unc_path
        );
    }

    info!("Share {} connected successfully", target.unc_path);
    Ok(())
}

pub fn generate_capture_filename(machine_name: &str) -> String {
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    format!("{machine_name}_{ts}.wim")
}

pub fn write_manifest(
    manifest: &CaptureManifest,
    log_dir: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(log_dir)
        .context("failed to create log directory")?;

    let filename = format!("capture_{}.json", manifest.id);
    let path = log_dir.join(&filename);

    let json = serde_json::to_string_pretty(manifest)
        .context("failed to serialize manifest")?;

    std::fs::write(&path, &json)
        .with_context(|| format!("failed to write manifest to {}", path.display()))?;

    info!("Capture manifest written to {}", path.display());
    Ok(path)
}

pub fn compute_file_hash(path: &Path) -> Result<String> {
    info!("Computing SHA-256 via certutil for {}", path.display());

    let output = Command::new("certutil")
        .args(["-hashfile", &path.to_string_lossy(), "SHA256"])
        .output()
        .context("failed to execute certutil")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("certutil failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash = stdout
        .lines()
        .nth(1)
        .map(|line| line.trim().replace(' ', ""))
        .unwrap_or_default();

    if hash.len() != 64 {
        warn!("certutil returned unexpected output: {stdout}");
        bail!("certutil did not return a valid SHA-256 hash");
    }

    info!("SHA-256: {hash}");
    Ok(hash)
}
