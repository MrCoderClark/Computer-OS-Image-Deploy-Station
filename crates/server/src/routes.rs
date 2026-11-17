use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use imaging_common::api::*;
use crate::state::AppState;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/api/auth", post(authenticate))
        .route("/api/images", get(list_images))
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
