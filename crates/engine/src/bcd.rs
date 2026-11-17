use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;
use tracing::info;

pub fn configure_boot(windows_volume: &str, esp_volume: &str) -> Result<()> {
    info!("Configuring BCD boot entry: Windows={windows_volume}, ESP={esp_volume}");

    let windows_dir = Path::new(windows_volume).join("Windows");
    if !windows_dir.exists() {
        bail!(
            "Windows directory not found at {} — image may not have been applied correctly",
            windows_dir.display()
        );
    }

    let output = Command::new("bcdboot")
        .arg(windows_dir.to_str().unwrap_or("W:\\Windows"))
        .arg("/s")
        .arg(esp_volume)
        .arg("/f")
        .arg("UEFI")
        .output()
        .context("failed to execute bcdboot")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    info!("bcdboot stdout: {stdout}");
    if !stderr.is_empty() {
        info!("bcdboot stderr: {stderr}");
    }

    if !output.status.success() {
        bail!("bcdboot failed (exit {}): {stderr}", output.status);
    }

    info!("Boot configuration written successfully");
    Ok(())
}
