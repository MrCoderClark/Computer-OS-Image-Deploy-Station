use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

use imaging_common::types::DeployStatus;

use crate::bcd;
use crate::disk::{self, PartitionResult};
use crate::wimgapi::WimSession;

#[derive(Debug, Clone)]
pub struct DeployRequest {
    pub job_id: Uuid,
    pub target_disk: u32,
    pub wim_path: PathBuf,
    pub image_index: u32,
}

#[derive(Debug, Clone)]
pub struct CaptureRequest {
    pub job_id: Uuid,
    pub source_volume: String,
    pub output_wim: PathBuf,
    pub machine_name: String,
    pub image_name: String,
    pub os_family: imaging_common::config::OsFamily,
    pub capture_target: Option<imaging_common::config::CaptureTarget>,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PipelineProgress {
    pub job_id: Uuid,
    pub status: DeployStatus,
    pub progress_pct: u8,
    pub message: String,
}

/// Validate the target disk and lay down the GPT partition table (ESP + MSR +
/// Windows). Split out from the full `run_deploy` so the agent can stage a
/// downloaded WIM onto the freshly-formatted Windows partition before apply --
/// WinPE's RAM-backed `X:\Temp` can't hold a real-size image, so the Windows
/// partition is the only disk-backed scratch we have.
pub async fn prepare_target_disk(
    request: &DeployRequest,
    progress_tx: &mpsc::Sender<PipelineProgress>,
) -> Result<PartitionResult> {
    let job_id = request.job_id;

    send_progress(progress_tx, job_id, DeployStatus::Partitioning, 0, "Validating target disk").await;

    let disks = tokio::task::spawn_blocking(disk::enumerate_disks)
        .await
        .context("disk enumeration task panicked")??;

    let target = disks
        .iter()
        .find(|d| d.number == request.target_disk)
        .ok_or_else(|| anyhow::anyhow!("Disk {} not found", request.target_disk))?;

    disk::validate_target_disk(target)?;

    send_progress(progress_tx, job_id, DeployStatus::Partitioning, 10, "Partitioning disk (GPT)").await;

    let disk_num = request.target_disk;
    let partition = tokio::task::spawn_blocking(move || disk::partition_gpt(disk_num))
        .await
        .context("partitioning task panicked")??;

    info!("Partitioned: ESP={}, Windows={}", partition.esp_volume, partition.windows_volume);

    Ok(partition)
}

/// Apply the WIM at `request.wim_path` onto the partition's Windows volume and
/// run bcdboot against the ESP. Call after `prepare_target_disk` (either
/// directly in the local-WIM fast path, or after a download into the Windows
/// partition for the network path).
pub async fn apply_and_configure(
    request: &DeployRequest,
    partition: &PartitionResult,
    progress_tx: &mpsc::Sender<PipelineProgress>,
) -> Result<()> {
    let job_id = request.job_id;

    send_progress(progress_tx, job_id, DeployStatus::Applying, 25, "Applying WIM image").await;

    let wim_path = request.wim_path.clone();
    let image_index = request.image_index;
    let windows_vol = partition.windows_volume.clone();
    tokio::task::spawn_blocking(move || {
        let session = WimSession::load()?;
        session.apply_image(&wim_path, image_index, Path::new(&windows_vol))
    })
    .await
    .context("WIM apply task panicked")??;

    send_progress(progress_tx, job_id, DeployStatus::Configuring, 85, "Configuring boot (bcdboot)").await;

    let esp = partition.esp_volume.clone();
    let win = partition.windows_volume.clone();
    tokio::task::spawn_blocking(move || bcd::configure_boot(&win, &esp))
        .await
        .context("bcdboot task panicked")??;

    send_progress(progress_tx, job_id, DeployStatus::Complete, 100, "Deploy complete").await;

    Ok(())
}

pub async fn run_deploy(
    request: DeployRequest,
    progress_tx: mpsc::Sender<PipelineProgress>,
) -> Result<()> {
    let partition = prepare_target_disk(&request, &progress_tx).await?;
    apply_and_configure(&request, &partition, &progress_tx).await
}

pub async fn run_capture(
    request: CaptureRequest,
    progress_tx: mpsc::Sender<PipelineProgress>,
) -> Result<()> {
    let job_id = request.job_id;

    // Step 1: Probe capture share
    if let Some(ref target) = request.capture_target {
        send_progress(&progress_tx, job_id, DeployStatus::Pending, 0, "Probing capture share").await;
        let target_clone = target.clone();
        tokio::task::spawn_blocking(move || crate::capture::probe_share(&target_clone))
            .await
            .context("share probe task panicked")??;
        send_progress(&progress_tx, job_id, DeployStatus::Pending, 5, "Capture share reachable").await;
    }

    // Step 2: Capture WIM
    send_progress(&progress_tx, job_id, DeployStatus::Applying, 10, "Capturing WIM image").await;

    let source = request.source_volume.clone();
    let output = request.output_wim.clone();
    tokio::task::spawn_blocking(move || {
        let session = WimSession::load()?;
        session.capture_image(&output, Path::new(&source))
    })
    .await
    .context("WIM capture task panicked")??;

    send_progress(&progress_tx, job_id, DeployStatus::Configuring, 70, "Computing file hash").await;

    // Step 3: Hash the captured WIM
    let wim_for_hash = request.output_wim.clone();
    let hash = tokio::task::spawn_blocking(move || {
        crate::capture::compute_file_hash(&wim_for_hash)
    })
    .await
    .context("hash task panicked")??;

    // Step 4: Get file size
    let file_size = std::fs::metadata(&request.output_wim)
        .map(|m| m.len())
        .unwrap_or(0);

    send_progress(&progress_tx, job_id, DeployStatus::Configuring, 85, "Writing capture manifest").await;

    // Step 5: Write local manifest
    let manifest = imaging_common::types::CaptureManifest {
        id: job_id,
        image_name: request.image_name.clone(),
        os_family: request.os_family,
        source_machine: request.machine_name.clone(),
        captured_at: chrono::Utc::now(),
        size_bytes: file_size,
        sha256: hash,
        unc_path: request.output_wim.to_string_lossy().to_string(),
    };

    let log_dir = request.log_dir.clone();
    tokio::task::spawn_blocking(move || crate::capture::write_manifest(&manifest, &log_dir))
        .await
        .context("manifest write task panicked")??;

    send_progress(&progress_tx, job_id, DeployStatus::Complete, 100, "Capture complete").await;

    Ok(())
}

async fn send_progress(
    tx: &mpsc::Sender<PipelineProgress>,
    job_id: Uuid,
    status: DeployStatus,
    pct: u8,
    message: &str,
) {
    let progress = PipelineProgress {
        job_id,
        status,
        progress_pct: pct,
        message: message.to_string(),
    };
    info!("[{job_id}] {status:?} {pct}% — {message}");
    let _ = tx.send(progress).await;
}
