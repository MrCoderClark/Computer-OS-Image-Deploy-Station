use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use imaging_common::api::*;
use imaging_common::driver::DriverPack;
use imaging_common::sequence::TaskSequence;
use uuid::Uuid;
use crate::state::AppState;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/auth", post(authenticate))
        .route("/api/images", get(list_images))
        .route("/api/images/{id}/download", get(download_image))
        .route("/api/deploy/status", post(report_deploy_status))
        .route("/api/captures/reconcile", post(reconcile_capture))
        .route("/api/sequences", get(list_sequences).post(save_sequence))
        .route("/api/sequences/{id}", get(get_sequence).delete(delete_sequence))
        .route("/api/drivers", get(list_driver_packs).post(save_driver_pack))
        .route("/api/drivers/{id}", delete(delete_driver_pack))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

async fn authenticate(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> Json<AuthResponse> {
    tracing::info!("Auth request from machine: {}", req.machine_name);
    let response = state.create_session(&req.machine_name).await;
    Json(response)
}

async fn list_images(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ImageListResponse>, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if state.validate_session(token).await.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let images = state.images.read().await;
    let summaries: Vec<ImageSummary> = images
        .iter()
        .map(|img| ImageSummary {
            id: img.id,
            name: img.name.clone(),
            os_family: img.os_family,
            captured_from: img.captured_from.clone(),
            build_date: Some(img.build_date.to_string()),
            size_bytes: img.size_bytes,
            sha256: img.sha256.clone(),
        })
        .collect();

    Ok(Json(ImageListResponse { images: summaries }))
}

#[derive(serde::Deserialize)]
struct DownloadQuery {
    #[serde(default)]
    transfer_id: Option<Uuid>,
    #[serde(default)]
    offset: Option<u64>,
}

async fn download_image(
    State(state): State<AppState>,
    Path(image_id): Path<Uuid>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if state.validate_session(token).await.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let images = state.images.read().await;
    let _image = images
        .iter()
        .find(|img| img.id == image_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let transfer_id = query.transfer_id.unwrap_or_else(Uuid::new_v4);
    let offset = query.offset.unwrap_or(0);

    tracing::info!(
        "Download requested: image={image_id}, transfer={transfer_id}, offset={offset}"
    );

    // TODO: Stream WIM file bytes from image store once real storage is wired up.
    // For now, acknowledge the request and return 204 (no content to serve yet).
    Ok(StatusCode::NO_CONTENT)
}

async fn report_deploy_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<DeployStatusUpdate>,
) -> Result<StatusCode, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if state.validate_session(token).await.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!(
        "Deploy status: job={} status={:?} {}% — {}",
        update.job_id,
        update.status,
        update.progress_pct,
        update.message,
    );

    Ok(StatusCode::OK)
}

async fn reconcile_capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(manifest): Json<imaging_common::types::CaptureManifest>,
) -> Result<StatusCode, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if state.validate_session(token).await.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!(
        "Capture reconciliation: id={}, image={}, machine={}, size={} bytes",
        manifest.id,
        manifest.image_name,
        manifest.source_machine,
        manifest.size_bytes,
    );

    let record = imaging_common::types::ImageRecord {
        id: manifest.id,
        name: manifest.image_name,
        os_family: manifest.os_family,
        captured_from: Some(manifest.source_machine),
        build_date: manifest.captured_at.date_naive(),
        size_bytes: manifest.size_bytes,
        sha256: manifest.sha256,
        created_at: manifest.captured_at,
    };

    state.images.write().await.push(record);

    tracing::info!("Capture registered in image catalogue");
    Ok(StatusCode::CREATED)
}

// --- Task Sequences ---

async fn list_sequences(
    State(state): State<AppState>,
) -> Json<Vec<TaskSequence>> {
    let seqs = state.sequences.read().await;
    Json(seqs.clone())
}

async fn get_sequence(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskSequence>, StatusCode> {
    let seqs = state.sequences.read().await;
    seqs.iter()
        .find(|s| s.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn save_sequence(
    State(state): State<AppState>,
    Json(mut seq): Json<TaskSequence>,
) -> Result<Json<TaskSequence>, (StatusCode, String)> {
    if let Err(errors) = seq.validate() {
        return Err((StatusCode::BAD_REQUEST, errors.join("; ")));
    }

    let mut seqs = state.sequences.write().await;
    if let Some(existing) = seqs.iter_mut().find(|s| s.id == seq.id) {
        seq.updated_at = chrono::Utc::now();
        *existing = seq.clone();
        tracing::info!("Updated task sequence: {}", seq.name);
    } else {
        let now = chrono::Utc::now();
        seq.created_at = now;
        seq.updated_at = now;
        seqs.push(seq.clone());
        tracing::info!("Created task sequence: {}", seq.name);
    }

    Ok(Json(seq))
}

async fn delete_sequence(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut seqs = state.sequences.write().await;
    let len_before = seqs.len();
    seqs.retain(|s| s.id != id);
    if seqs.len() < len_before {
        tracing::info!("Deleted task sequence: {id}");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// --- Driver Packs ---

async fn list_driver_packs(
    State(state): State<AppState>,
) -> Json<Vec<DriverPack>> {
    let packs = state.driver_packs.read().await;
    Json(packs.clone())
}

async fn save_driver_pack(
    State(state): State<AppState>,
    Json(mut pack): Json<DriverPack>,
) -> Json<DriverPack> {
    let mut packs = state.driver_packs.write().await;
    if let Some(existing) = packs.iter_mut().find(|p| p.id == pack.id) {
        *existing = pack.clone();
        tracing::info!("Updated driver pack: {}", pack.name);
    } else {
        pack.created_at = chrono::Utc::now();
        packs.push(pack.clone());
        tracing::info!("Created driver pack: {}", pack.name);
    }
    Json(pack)
}

async fn delete_driver_pack(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut packs = state.driver_packs.write().await;
    let len_before = packs.len();
    packs.retain(|p| p.id != id);
    if packs.len() < len_before {
        tracing::info!("Deleted driver pack: {id}");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
