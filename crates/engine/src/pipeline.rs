use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

use imaging_common::types::DeployStatus;

use crate::bcd;
use crate::disk;
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
}

#[derive(Debug, Clone)]
pub struct PipelineProgress {
    pub job_id: Uuid,
    pub status: DeployStatus,
    pub progress_pct: u8,
    pub message: String,
}

pub async fn run_deploy(
    request: DeployRequest,
    progress_tx: mpsc::Sender<PipelineProgress>,
) -> Result<()> {
    let job_id = request.job_id;

    send_progress(&progress_tx, job_id, DeployStatus::Partitioning, 0, "Validating target disk").await;

    let disks = tokio::task::spawn_blocking(disk::enumerate_disks)
        .await
        .context("disk enumeration task panicked")??;

    let target = disks
        .iter()
        .find(|d| d.number == request.target_disk)
        .ok_or_else(|| anyhow::anyhow!("Disk {} not found", request.target_disk))?;

    disk::validate_target_disk(target)?;

    send_progress(&progress_tx, job_id, DeployStatus::Partitioning, 10, "Partitioning disk (GPT)").await;

    let disk_num = request.target_disk;
    let partition = tokio::task::spawn_blocking(move || disk::partition_gpt(disk_num))
        .await
        .context("partitioning task panicked")??;

    info!("Partitioned: ESP={}, Windows={}", partition.esp_volume, partition.windows_volume);

    send_progress(&progress_tx, job_id, DeployStatus::Applying, 25, "Applying WIM image").await;

    let wim_path = request.wim_path.clone();
    let image_index = request.image_index;
    let windows_vol = partition.windows_volume.clone();
    tokio::task::spawn_blocking(move || {
        let session = WimSession::load()?;
        session.apply_image(&wim_path, image_index, Path::new(&windows_vol))
    })
    .await
    .context("WIM apply task panicked")??;

    send_progress(&progress_tx, job_id, DeployStatus::Configuring, 85, "Configuring boot (bcdboot)").await;

    let esp = partition.esp_volume.clone();
    let win = partition.windows_volume.clone();
    tokio::task::spawn_blocking(move || bcd::configure_boot(&win, &esp))
        .await
        .context("bcdboot task panicked")??;

    send_progress(&progress_tx, job_id, DeployStatus::Complete, 100, "Deploy complete").await;

    Ok(())
}

pub async fn run_capture(
    request: CaptureRequest,
    progress_tx: mpsc::Sender<PipelineProgress>,
) -> Result<()> {
    let job_id = request.job_id;

    send_progress(&progress_tx, job_id, DeployStatus::Applying, 0, "Preparing capture").await;

    let source = request.source_volume.clone();
    let output = request.output_wim.clone();
    send_progress(&progress_tx, job_id, DeployStatus::Applying, 10, "Capturing WIM image").await;

    tokio::task::spawn_blocking(move || {
        let session = WimSession::load()?;
        session.capture_image(&output, Path::new(&source))
    })
    .await
    .context("WIM capture task panicked")??;

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
