use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use imaging_common::api::{AuthResponse, SessionClaims};
use imaging_common::driver::DriverPack;
use imaging_common::sequence::TaskSequence;
use imaging_common::types::ImageRecord;

#[derive(Clone)]
pub struct AppState {
    pub images: Arc<RwLock<Vec<ImageRecord>>>,
    pub sessions: Arc<RwLock<HashMap<String, SessionClaims>>>,
    pub sequences: Arc<RwLock<Vec<TaskSequence>>>,
    pub driver_packs: Arc<RwLock<Vec<DriverPack>>>,
}

impl AppState {
    pub fn new_with_samples() -> Self {
        let images = vec![
            ImageRecord {
                id: Uuid::new_v4(),
                name: "Win11-24H2-Base".into(),
                os_family: imaging_common::config::OsFamily::Win11,
                captured_from: Some("HP EliteBook 840 G9".into()),
                build_date: chrono::NaiveDate::from_ymd_opt(2026, 11, 10).unwrap(),
                size_bytes: 21_474_836_480,
                sha256: "a1b2c3d4e5f6...".into(),
                created_at: chrono::Utc::now(),
            },
            ImageRecord {
                id: Uuid::new_v4(),
                name: "Win11-24H2-Dev".into(),
                os_family: imaging_common::config::OsFamily::Win11,
                captured_from: Some("Dell Latitude 5540".into()),
                build_date: chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(),
                size_bytes: 26_306_043_904,
                sha256: "f6e5d4c3b2a1...".into(),
                created_at: chrono::Utc::now(),
            },
            ImageRecord {
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

        Self {
            images: Arc::new(RwLock::new(images)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            sequences: Arc::new(RwLock::new(Vec::new())),
            driver_packs: Arc::new(RwLock::new(Vec::new())),
        }
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
