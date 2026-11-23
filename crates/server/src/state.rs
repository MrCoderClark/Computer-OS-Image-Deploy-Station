use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use imaging_common::api::{AuthResponse, SessionClaims};
use imaging_common::types::AuditLevel;

use crate::db::{self, Database};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub sessions: Arc<RwLock<HashMap<String, SessionClaims>>>,
    /// PEM-encoded server cert (public bytes only) and its SHA-256
    /// fingerprint. Populated when TLS is enabled; `None` when the server
    /// runs in plain-HTTP mode. Exposed via /cert and /fingerprint so
    /// admin clients can bootstrap pinning.
    pub tls_cert_pem: Option<String>,
    pub tls_fingerprint: Option<String>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            tls_cert_pem: None,
            tls_fingerprint: None,
        }
    }

    pub fn with_tls(mut self, cert_pem: String, fingerprint: String) -> Self {
        self.tls_cert_pem = Some(cert_pem);
        self.tls_fingerprint = Some(fingerprint);
        self
    }

    pub fn seed_sample_images(&self) {
        let db = self.db.blocking_lock();
        let samples = vec![
            imaging_common::types::ImageRecord {
                id: Uuid::new_v4(),
                name: "Win11-24H2-Base".into(),
                os_family: imaging_common::config::OsFamily::Win11,
                captured_from: Some("HP EliteBook 840 G9".into()),
                build_date: chrono::NaiveDate::from_ymd_opt(2026, 11, 10).unwrap(),
                size_bytes: 21_474_836_480,
                sha256: "a1b2c3d4e5f6...".into(),
                created_at: chrono::Utc::now(),
            },
            imaging_common::types::ImageRecord {
                id: Uuid::new_v4(),
                name: "Win11-24H2-Dev".into(),
                os_family: imaging_common::config::OsFamily::Win11,
                captured_from: Some("Dell Latitude 5540".into()),
                build_date: chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(),
                size_bytes: 26_306_043_904,
                sha256: "f6e5d4c3b2a1...".into(),
                created_at: chrono::Utc::now(),
            },
            imaging_common::types::ImageRecord {
                id: Uuid::new_v4(),
                name: "Win10-22H2-Legacy".into(),
                os_family: imaging_common::config::OsFamily::Win10,
                captured_from: Some("Lenovo ThinkPad T14".into()),
                build_date: chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap(),
                size_bytes: 19_541_065_728,
                sha256: "deadbeef1234...".into(),
                created_at: chrono::Utc::now(),
            },
        ];

        for img in &samples {
            if let Err(e) = db.insert_image(img) {
                tracing::warn!("Failed to seed image: {e}");
            }
        }

        db::audit(&db, "system", "server_start", "imaging-server", Some("Seeded sample images"), AuditLevel::Info);
    }

    pub async fn create_session(&self, machine_name: &str) -> AuthResponse {
        let token = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::hours(8);

        let claims = SessionClaims {
            machine_name: machine_name.into(),
            issued_at: now,
            expires_at: expires,
        };

        self.sessions.write().await.insert(token.clone(), claims);

        AuthResponse {
            session_token: token,
            expires_at: expires,
        }
    }

    pub async fn validate_session(&self, token: &str) -> Option<SessionClaims> {
        let sessions = self.sessions.read().await;
        sessions.get(token).and_then(|claims| {
            if claims.expires_at > chrono::Utc::now() {
                Some(claims.clone())
            } else {
                None
            }
        })
    }
}
