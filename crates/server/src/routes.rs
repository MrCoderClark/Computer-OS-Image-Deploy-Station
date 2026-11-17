use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use imaging_common::api::*;
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
