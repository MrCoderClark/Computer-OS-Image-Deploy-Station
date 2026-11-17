use anyhow::{Context, Result, bail};
use std::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub number: u32,
    pub size_bytes: u64,
    pub model: String,
    pub is_boot: bool,
}

#[derive(Debug, Clone)]
pub struct PartitionResult {
    pub esp_volume: String,
    pub msr_offset: u64,
    pub windows_volume: String,
}

pub fn enumerate_disks() -> Result<Vec<DiskInfo>> {
    let output = run_diskpart_script(
        "list disk\n"
    )?;
    info!("Disk enumeration output:\n{output}");
    parse_disk_list(&output)
}

pub fn partition_gpt(disk_number: u32) -> Result<PartitionResult> {
    info!("Partitioning disk {disk_number} with GPT layout (ESP + MSR + Windows)");

    let script = format!(
        r#"select disk {disk_number}
clean
convert gpt
create partition efi size=300
format fs=fat32 quick label="System"
assign letter=S
create partition msr size=16
create partition primary
format fs=ntfs quick label="Windows"
assign letter=W
"#
    );

    let output = run_diskpart_script(&script)
        .context("diskpart partitioning failed")?;
    info!("Diskpart output:\n{output}");

    if output.contains("DiskPart has encountered an error") {
        bail!("Diskpart reported an error:\n{output}");
    }

    Ok(PartitionResult {
        esp_volume: "S:".into(),
        msr_offset: 300 * 1024 * 1024 + 16 * 1024 * 1024,
        windows_volume: "W:".into(),
    })
}

pub fn validate_target_disk(disk: &DiskInfo) -> Result<()> {
    if disk.is_boot {
        bail!(
            "Disk {} ({}) is the boot device — refusing to overwrite",
            disk.number,
            disk.model
        );
    }
    if disk.size_bytes < 20 * 1024 * 1024 * 1024 {
        warn!(
            "Disk {} is only {:.1} GB — may be too small for a Windows image",
            disk.number,
            disk.size_bytes as f64 / 1_073_741_824.0
        );
    }
    Ok(())
}

fn run_diskpart_script(script: &str) -> Result<String> {
    let temp_path = std::env::temp_dir().join("deploystation_diskpart.txt");
    std::fs::write(&temp_path, script)
        .context("failed to write diskpart script")?;

    let output = Command::new("diskpart")
        .arg("/s")
        .arg(&temp_path)
        .output()
        .context("failed to execute diskpart")?;

    let _ = std::fs::remove_file(&temp_path);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        bail!("diskpart exited with {}: {stderr}", output.status);
    }

    Ok(stdout)
}

fn parse_disk_list(output: &str) -> Result<Vec<DiskInfo>> {
    let mut disks = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Disk ") || trimmed.starts_with("Disk ###") {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let number: u32 = match parts[1].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let size_str = parts.get(3).unwrap_or(&"0");
        let unit = parts.get(4).unwrap_or(&"GB");
        let size_val: f64 = size_str.parse().unwrap_or(0.0);
        let size_bytes = match *unit {
            "GB" => (size_val * 1_073_741_824.0) as u64,
            "MB" => (size_val * 1_048_576.0) as u64,
            "TB" => (size_val * 1_099_511_627_776.0) as u64,
            _ => (size_val * 1_073_741_824.0) as u64,
        };

        let is_boot = trimmed.contains("*");

        disks.push(DiskInfo {
            number,
            size_bytes,
            model: format!("Disk {number}"),
            is_boot,
        });
    }

    Ok(disks)
}
