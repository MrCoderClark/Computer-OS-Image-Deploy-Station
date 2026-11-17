use anyhow::{Context, Result};
use std::path::Path;

pub fn inject_drivers(windows_volume: &str, driver_path: &Path) -> Result<()> {
    if !driver_path.exists() {
        anyhow::bail!(
            "Driver path does not exist: {}",
            driver_path.display()
        );
    }

    let image_path = format!(r"{windows_volume}\");

    tracing::info!(
        "Injecting drivers from {} into {}",
        driver_path.display(),
        image_path
    );

    let output = std::process::Command::new("dism")
        .args([
            "/Image", &image_path,
            "/Add-Driver",
            &format!("/Driver:{}", driver_path.display()),
            "/Recurse",
        ])
        .output()
        .context("Failed to execute DISM")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    tracing::debug!("DISM stdout: {stdout}");

    if !output.status.success() {
        anyhow::bail!("DISM driver injection failed: {stderr}");
    }

    let count = stdout
        .lines()
        .filter(|l| l.contains("Successfully installed"))
        .count();

    tracing::info!("Driver injection complete — {count} driver(s) installed");
    Ok(())
}

pub fn list_drivers_in_path(driver_path: &Path) -> Result<Vec<DriverFileInfo>> {
    let mut drivers = Vec::new();

    if !driver_path.exists() {
        return Ok(drivers);
    }

    fn walk(dir: &Path, out: &mut Vec<DriverFileInfo>) -> Result<()> {
        for entry in std::fs::read_dir(dir).context("read driver dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("inf") {
                out.push(DriverFileInfo {
                    inf_path: path.to_string_lossy().to_string(),
                    file_name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                });
            }
        }
        Ok(())
    }

    walk(driver_path, &mut drivers)?;
    tracing::info!("Found {} .inf files in {}", drivers.len(), driver_path.display());
    Ok(drivers)
}

#[derive(Debug, Clone)]
pub struct DriverFileInfo {
    pub inf_path: String,
    pub file_name: String,
}
