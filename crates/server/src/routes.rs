use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use imaging_common::api::*;
use imaging_common::driver::DriverPack;
use imaging_common::security::{self, CreateKeyRequest, CreateKeyResponse, EnrollmentKey, KeyListEntry};
use imaging_common::sequence::TaskSequence;
use imaging_common::types::AuditLevel;
use uuid::Uuid;

use crate::db;
use crate::state::AppState;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/cert", get(server_cert))
        .route("/fingerprint", get(server_fingerprint))
        .route("/api/auth", post(authenticate))
        .route("/api/images", get(list_images))
        .route("/api/images/{id}/download", get(download_image))
        .route("/api/deploy/status", post(report_deploy_status))
        .route("/api/deploy/jobs", get(list_deploy_jobs))
        .route("/api/captures/reconcile", post(reconcile_capture))
        .route("/api/sequences", get(list_sequences).post(save_sequence))
        .route("/api/sequences/{id}", get(get_sequence).delete(delete_sequence))
        .route("/api/drivers", get(list_driver_packs).post(save_driver_pack))
        .route("/api/drivers/{id}", delete(delete_driver_pack))
        .route("/api/audit", get(query_audit_log))
        .route("/api/audit/export", get(export_audit_csv))
        .route("/api/keys", get(list_keys).post(create_key))
        .route("/api/keys/{id}/revoke", post(revoke_key))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

/// PEM-encoded server certificate. Unauthenticated: the cert is public
/// information and MITM resistance comes from the admin confirming the
/// fingerprint out-of-band before trusting the response.
async fn server_cert(
    State(state): State<AppState>,
) -> Result<(axum::http::HeaderMap, String), (StatusCode, String)> {
    let pem = state.tls_cert_pem.clone()
        .ok_or((StatusCode::NOT_FOUND, "TLS disabled on this server".into()))?;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, "application/x-pem-file".parse().unwrap());
    Ok((headers, pem))
}

/// SHA-256 fingerprint of the server cert (colon-separated hex). Admin
/// displays this at first-trust and the user verifies it out-of-band.
async fn server_fingerprint(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let fp = state.tls_fingerprint.clone()
        .ok_or((StatusCode::NOT_FOUND, "TLS disabled on this server".into()))?;
    Ok(Json(serde_json::json!({ "sha256": fp })))
}

async fn authenticate(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    tracing::info!("Auth request from machine: {}", req.machine_name);

    let db = state.db.lock().await;

    if let Some(raw_key) = &req.enrollment_key {
        let hash = security::hash_key(raw_key);
        match db.validate_enrollment_key(&hash) {
            Ok(Some(key)) if key.is_valid() => {
                db::audit(&db, &req.machine_name, "auth", &req.machine_name, Some(&format!("key={}", key.label)), AuditLevel::Info);
            }
            Ok(Some(key)) => {
                let reason = if key.revoked { "revoked" } else { "expired" };
                db::audit(&db, &req.machine_name, "auth_rejected", &req.machine_name, Some(&format!("key={} ({})", key.label, reason)), AuditLevel::Warning);
                return Err((StatusCode::UNAUTHORIZED, format!("Enrollment key is {reason}")));
            }
            Ok(None) => {
                db::audit(&db, &req.machine_name, "auth_rejected", &req.machine_name, Some("unknown key"), AuditLevel::Warning);
                return Err((StatusCode::UNAUTHORIZED, "Invalid enrollment key".into()));
            }
            Err(e) => {
                tracing::error!("Key lookup failed: {e}");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Key validation error".into()));
            }
        }
    } else {
        let has_keys = db.list_enrollment_keys().map(|k| !k.is_empty()).unwrap_or(false);
        if has_keys {
            db::audit(&db, &req.machine_name, "auth_rejected", &req.machine_name, Some("no key provided"), AuditLevel::Warning);
            return Err((StatusCode::UNAUTHORIZED, "Enrollment key required".into()));
        }
        db::audit(&db, &req.machine_name, "auth", &req.machine_name, Some("no keys configured"), AuditLevel::Info);
    }

    drop(db);
    let response = state.create_session(&req.machine_name).await;
    Ok(Json(response))
}

async fn list_images(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ImageListResponse>, StatusCode> {
    extract_session(&state, &headers).await?;

    let db = state.db.lock().await;
    let images = db.list_images().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    extract_session(&state, &headers).await?;

    let transfer_id = query.transfer_id.unwrap_or_else(Uuid::new_v4);
    let offset = query.offset.unwrap_or(0);

    tracing::info!(
        "Download requested: image={image_id}, transfer={transfer_id}, offset={offset}"
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn report_deploy_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<DeployStatusUpdate>,
) -> Result<StatusCode, StatusCode> {
    let claims = extract_session(&state, &headers).await?;

    tracing::info!(
        "Deploy status: job={} status={:?} {}% — {}",
        update.job_id, update.status, update.progress_pct, update.message,
    );

    let job = imaging_common::types::DeployJob {
        id: update.job_id,
        machine_name: claims.machine_name.clone(),
        image_id: Uuid::nil(),
        status: update.status,
        progress_pct: update.progress_pct,
        started_at: chrono::Utc::now(),
        started_by: claims.machine_name.clone(),
    };

    let db = state.db.lock().await;
    db.upsert_deploy_job(&job, &update.message).ok();
    db::audit(&db, &claims.machine_name, "deploy_status", &update.job_id.to_string(), Some(&update.message), AuditLevel::Info);

    Ok(StatusCode::OK)
}

async fn list_deploy_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<db::DeployJobRow>>, StatusCode> {
    extract_session(&state, &headers).await?;

    let db = state.db.lock().await;
    let jobs = db.list_deploy_jobs().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(jobs))
}

async fn reconcile_capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(manifest): Json<imaging_common::types::CaptureManifest>,
) -> Result<StatusCode, StatusCode> {
    let claims = extract_session(&state, &headers).await?;

    tracing::info!(
        "Capture reconciliation: id={}, image={}, machine={}, size={} bytes",
        manifest.id, manifest.image_name, manifest.source_machine, manifest.size_bytes,
    );

    let record = imaging_common::types::ImageRecord {
        id: manifest.id,
        name: manifest.image_name.clone(),
        os_family: manifest.os_family,
        captured_from: Some(manifest.source_machine.clone()),
        build_date: manifest.captured_at.date_naive(),
        size_bytes: manifest.size_bytes,
        sha256: manifest.sha256,
        created_at: manifest.captured_at,
    };

    let db = state.db.lock().await;
    db.insert_image(&record).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db::audit(&db, &claims.machine_name, "capture_reconcile", &manifest.image_name, Some(&manifest.source_machine), AuditLevel::Info);

    tracing::info!("Capture registered in image catalogue");
    Ok(StatusCode::CREATED)
}

// --- Task Sequences ---

async fn list_sequences(
    State(state): State<AppState>,
) -> Result<Json<Vec<TaskSequence>>, StatusCode> {
    let db = state.db.lock().await;
    let seqs = db.list_sequences().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(seqs))
}

async fn get_sequence(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskSequence>, StatusCode> {
    let db = state.db.lock().await;
    let seqs = db.list_sequences().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    seqs.into_iter()
        .find(|s| s.id == id)
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

    seq.updated_at = chrono::Utc::now();
    let db = state.db.lock().await;
    db.upsert_sequence(&seq).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::audit(&db, "admin", "save_sequence", &seq.name, None, AuditLevel::Info);

    Ok(Json(seq))
}

async fn delete_sequence(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state.db.lock().await;
    let deleted = db.delete_sequence(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        db::audit(&db, "admin", "delete_sequence", &id.to_string(), None, AuditLevel::Warning);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// --- Driver Packs ---

async fn list_driver_packs(
    State(state): State<AppState>,
) -> Result<Json<Vec<DriverPack>>, StatusCode> {
    let db = state.db.lock().await;
    let packs = db.list_driver_packs().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(packs))
}

async fn save_driver_pack(
    State(state): State<AppState>,
    Json(mut pack): Json<DriverPack>,
) -> Result<Json<DriverPack>, StatusCode> {
    pack.created_at = chrono::Utc::now();
    let db = state.db.lock().await;
    db.upsert_driver_pack(&pack).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db::audit(&db, "admin", "save_driver_pack", &pack.name, None, AuditLevel::Info);
    Ok(Json(pack))
}

async fn delete_driver_pack(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state.db.lock().await;
    let deleted = db.delete_driver_pack(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        db::audit(&db, "admin", "delete_driver_pack", &id.to_string(), None, AuditLevel::Warning);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// --- Enrollment Keys ---

async fn create_key(
    State(state): State<AppState>,
    Json(req): Json<CreateKeyRequest>,
) -> Result<Json<CreateKeyResponse>, (StatusCode, String)> {
    let plaintext = security::generate_key();
    let hash = security::hash_key(&plaintext);
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::days(req.expires_in_days as i64);

    let key = EnrollmentKey {
        id: Uuid::new_v4(),
        label: req.label.clone(),
        key_hash: hash,
        created_at: now,
        expires_at,
        revoked: false,
        revoked_at: None,
        created_by: "admin".into(),
    };

    let db = state.db.lock().await;
    db.insert_enrollment_key(&key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::audit(&db, "admin", "create_key", &key.label, Some(&format!("expires={}", expires_at.to_rfc3339())), AuditLevel::Info);

    Ok(Json(CreateKeyResponse {
        id: key.id,
        label: key.label,
        plaintext_key: plaintext,
        expires_at,
    }))
}

async fn list_keys(
    State(state): State<AppState>,
) -> Result<Json<Vec<KeyListEntry>>, StatusCode> {
    let db = state.db.lock().await;
    let keys = db.list_enrollment_keys().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let entries: Vec<KeyListEntry> = keys
        .iter()
        .map(|k| KeyListEntry {
            id: k.id,
            label: k.label.clone(),
            created_at: k.created_at,
            expires_at: k.expires_at,
            revoked: k.revoked,
            days_remaining: k.days_until_expiry(),
            created_by: k.created_by.clone(),
        })
        .collect();

    Ok(Json(entries))
}

async fn revoke_key(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state.db.lock().await;
    let revoked = db.revoke_enrollment_key(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if revoked {
        db::audit(&db, "admin", "revoke_key", &id.to_string(), None, AuditLevel::Warning);
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// --- Audit Log ---

#[derive(serde::Deserialize)]
struct AuditQuery {
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 { 200 }

async fn query_audit_log(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<imaging_common::types::AuditEntry>>, StatusCode> {
    extract_session(&state, &headers).await?;

    let db = state.db.lock().await;
    let entries = db.query_audit(q.search.as_deref(), q.level.as_deref(), q.limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(entries))
}

async fn export_audit_csv(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Result<(HeaderMap, String), StatusCode> {
    extract_session(&state, &headers).await?;

    let db = state.db.lock().await;
    let entries = db.query_audit(q.search.as_deref(), q.level.as_deref(), q.limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut csv = String::from("Timestamp,Actor,Action,Target,Detail,Level\n");
    for e in &entries {
        let detail = e.detail.as_deref().unwrap_or("");
        csv.push_str(&format!(
            "{},{},{},{},\"{}\",{:?}\n",
            e.timestamp.to_rfc3339(), e.actor, e.action, e.target, detail.replace('"', "\"\""), e.level
        ));
    }

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert("content-type", "text/csv".parse().unwrap());
    resp_headers.insert("content-disposition", "attachment; filename=\"audit_log.csv\"".parse().unwrap());
    Ok((resp_headers, csv))
}

// --- Helpers ---

async fn extract_session(state: &AppState, headers: &HeaderMap) -> Result<SessionClaims, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    state.validate_session(token).await.ok_or(StatusCode::UNAUTHORIZED)
}
