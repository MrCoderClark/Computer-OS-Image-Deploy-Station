use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

pub fn configure_boot(windows_volume: &str, esp_volume: &str) -> Result<()> {
    info!("Configuring BCD boot entry: Windows={windows_volume}, ESP={esp_volume}");

    let windows_dir = Path::new(windows_volume).join("Windows");
    if !windows_dir.exists() {
        bail!(
            "Windows directory not found at {} — image may not have been applied correctly",
            windows_dir.display()
        );
    }

    // Probe the applied image for the files bcdboot depends on. For UEFI
    // targets the critical ones are:
    //   <W>\Windows\Boot\EFI\bootmgfw.efi      (gets copied to S:\EFI\Microsoft\Boot\)
    //   <W>\Windows\System32\config\BCD-Template  (the "BCD template store")
    // bcdboot's "BCD template store [c000000f]" error happens when that
    // second one is missing / wrong format -- very common on captured WIMs
    // that were taken from a sysprepped install stripped of boot assets.
    let probe = BootAssetProbe::collect(&windows_dir);
    info!("Boot asset probe:\n{probe}");
    if probe.has_critical_gaps() {
        warn!("bcdboot is about to run against an image missing required boot assets; expect failure");
    }

    // /v gets us the actual culprit path when bcdboot refuses, rather than
    // the one-line "BFSVC Error" that leaves you guessing.
    let output = Command::new("bcdboot")
        .arg(windows_dir.to_str().unwrap_or("W:\\Windows"))
        .arg("/s").arg(esp_volume)
        .arg("/f").arg("UEFI")
        .arg("/v")
        .output()
        .context("failed to execute bcdboot")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !stdout.is_empty() { info!("bcdboot stdout:\n{stdout}"); }
    if !stderr.is_empty() { info!("bcdboot stderr:\n{stderr}"); }

    if !output.status.success() {
        let exit = output.status.code()
            .map(|c| format!("exit code: {c}"))
            .unwrap_or_else(|| "terminated by signal".into());

        // The error message surfaces to the agent's UI via {e:#}. Make it
        // tell the operator WHY bcdboot refused, not just that it did.
        let combined = if stderr.is_empty() { stdout.clone() } else { stderr.clone() };
        let diag = probe.failure_hint();
        bail!(
            "bcdboot failed ({exit})\n\
             bcdboot said: {combined}\n\
             boot assets on {}:\n{}{}",
            windows_dir.display(),
            probe,
            if diag.is_empty() { String::new() } else { format!("\n{diag}") }
        );
    }

    info!("Boot configuration written successfully");
    Ok(())
}

struct BootAssetProbe {
    bootmgfw_efi: Asset,
    bcd_template: Asset,
    boot_dir: Asset,
    config_dir: Asset,
}

struct Asset {
    path: PathBuf,
    kind: AssetKind,
    status: AssetStatus,
}

enum AssetKind { File, Dir }

enum AssetStatus {
    Missing,
    File { size_bytes: u64 },
    Dir { entries: usize },
    ProbeError(String),
}

impl BootAssetProbe {
    fn collect(windows_dir: &Path) -> Self {
        // Windows FS is case-insensitive, so either case works for lookup.
        // We write the canonical names here for log clarity.
        let bootmgfw_efi = Asset::new(
            windows_dir.join("Boot").join("EFI").join("bootmgfw.efi"), AssetKind::File);
        let bcd_template = Asset::new(
            windows_dir.join("System32").join("config").join("BCD-Template"), AssetKind::File);
        let boot_dir = Asset::new(windows_dir.join("Boot"), AssetKind::Dir);
        let config_dir = Asset::new(windows_dir.join("System32").join("config"), AssetKind::Dir);
        Self { bootmgfw_efi, bcd_template, boot_dir, config_dir }
    }

    /// True when at least one bcdboot-critical file is missing or zero-byte.
    fn has_critical_gaps(&self) -> bool {
        matches!(self.bootmgfw_efi.status,
                 AssetStatus::Missing | AssetStatus::File { size_bytes: 0 })
        || matches!(self.bcd_template.status,
                    AssetStatus::Missing | AssetStatus::File { size_bytes: 0 })
    }

    /// A one-liner explaining the likely root cause from the probe. Empty
    /// when nothing obvious stands out.
    fn failure_hint(&self) -> String {
        match (&self.bootmgfw_efi.status, &self.bcd_template.status) {
            (AssetStatus::Missing, AssetStatus::Missing) => {
                "Both bootmgfw.efi and BCD-Template are missing. The WIM likely does not include \
                 a bootable Windows install (e.g. a Sysprep /generalize WIM stripped of boot \
                 assets, or a partial capture). Re-capture with the Boot folder included, or \
                 use an install.wim from Windows media.".into()
            }
            (_, AssetStatus::Missing) => {
                "BCD-Template is missing. bcdboot needs \
                 Windows\\System32\\config\\BCD-Template to seed the ESP's BCD store. \
                 The source WIM may be a specialized or partial capture. Try an install.wim \
                 from Windows media, or restore the file from a reference install and re-capture.".into()
            }
            (AssetStatus::Missing, _) => {
                "bootmgfw.efi is missing from Windows\\Boot\\EFI. The capture did not include \
                 UEFI boot files. Re-capture with the Boot folder, or use an install.wim.".into()
            }
            _ => String::new(),
        }
    }
}

impl Asset {
    fn new(path: PathBuf, kind: AssetKind) -> Self {
        let status = match &kind {
            AssetKind::File => match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => AssetStatus::File { size_bytes: m.len() },
                Ok(_)  => AssetStatus::ProbeError("exists but is not a file".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => AssetStatus::Missing,
                Err(e) => AssetStatus::ProbeError(e.to_string()),
            },
            AssetKind::Dir => match std::fs::read_dir(&path) {
                Ok(iter) => AssetStatus::Dir { entries: iter.count() },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => AssetStatus::Missing,
                Err(e) => AssetStatus::ProbeError(e.to_string()),
            },
        };
        Self { path, kind, status }
    }
}

impl std::fmt::Display for BootAssetProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  {}", self.bootmgfw_efi)?;
        writeln!(f, "  {}", self.bcd_template)?;
        writeln!(f, "  {}", self.boot_dir)?;
        write!  (f, "  {}", self.config_dir)
    }
}

impl std::fmt::Display for Asset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.kind {
            AssetKind::File => "file",
            AssetKind::Dir  => "dir",
        };
        write!(f, "{} ({}) : {}", self.path.display(), label, self.status)
    }
}

impl std::fmt::Display for AssetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "MISSING"),
            Self::File { size_bytes: 0 } => write!(f, "present but ZERO BYTES"),
            Self::File { size_bytes } => write!(f, "present, {} bytes", size_bytes),
            Self::Dir { entries } => write!(f, "present, {} entries", entries),
            Self::ProbeError(e) => write!(f, "probe error: {e}"),
        }
    }
}
