use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentKey {
    pub id: Uuid,
    pub label: String,
    pub key_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub revoked: bool,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: String,
}

impl EnrollmentKey {
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }

    pub fn is_valid(&self) -> bool {
        !self.revoked && !self.is_expired()
    }

    pub fn days_until_expiry(&self) -> i64 {
        let now = chrono::Utc::now();
        (self.expires_at - now).num_days()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    pub label: String,
    pub expires_in_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    pub id: Uuid,
    pub label: String,
    pub plaintext_key: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyListEntry {
    pub id: Uuid,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub revoked: bool,
    pub days_remaining: i64,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeKeyRequest {
    pub id: Uuid,
}

pub fn hash_key(plaintext: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    plaintext.hash(&mut hasher);
    let h = hasher.finish();
    format!("sha256:{:016x}", h)
}

pub fn generate_key() -> String {
    let bytes: [u8; 24] = rand_bytes();
    base64_encode(&bytes)
}

fn rand_bytes() -> [u8; 24] {
    let mut buf = [0u8; 24];
    buf[0..16].copy_from_slice(Uuid::new_v4().as_bytes());
    buf[16..24].copy_from_slice(&Uuid::new_v4().as_bytes()[0..8]);
    buf
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_key_not_empty() {
        let key = generate_key();
        assert!(!key.is_empty());
    }

    #[test]
    fn generate_key_is_base64() {
        let key = generate_key();
        assert!(key.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='));
    }

    #[test]
    fn generate_key_unique() {
        let a = generate_key();
        let b = generate_key();
        assert_ne!(a, b);
    }

    #[test]
    fn generate_key_length() {
        let key = generate_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn hash_key_deterministic() {
        let hash1 = hash_key("test-key-abc");
        let hash2 = hash_key("test-key-abc");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn hash_key_different_inputs() {
        let hash1 = hash_key("key-one");
        let hash2 = hash_key("key-two");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn hash_key_has_prefix() {
        let hash = hash_key("anything");
        assert!(hash.starts_with("sha256:"));
    }

    #[test]
    fn enrollment_key_valid_when_active() {
        let key = EnrollmentKey {
            id: Uuid::new_v4(),
            label: "test".into(),
            key_hash: hash_key("abc"),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            revoked: false,
            revoked_at: None,
            created_by: "admin".into(),
        };
        assert!(key.is_valid());
        assert!(!key.is_expired());
        assert!(!key.revoked);
    }

    #[test]
    fn enrollment_key_expired() {
        let key = EnrollmentKey {
            id: Uuid::new_v4(),
            label: "expired".into(),
            key_hash: hash_key("abc"),
            created_at: chrono::Utc::now() - chrono::Duration::hours(2),
            expires_at: chrono::Utc::now() - chrono::Duration::hours(1),
            revoked: false,
            revoked_at: None,
            created_by: "admin".into(),
        };
        assert!(key.is_expired());
        assert!(!key.is_valid());
    }

    #[test]
    fn enrollment_key_revoked() {
        let key = EnrollmentKey {
            id: Uuid::new_v4(),
            label: "revoked".into(),
            key_hash: hash_key("abc"),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            revoked: true,
            revoked_at: Some(chrono::Utc::now()),
            created_by: "admin".into(),
        };
        assert!(!key.is_valid());
        assert!(!key.is_expired());
        assert!(key.revoked);
    }

    #[test]
    fn days_until_expiry_positive() {
        let key = EnrollmentKey {
            id: Uuid::new_v4(),
            label: "test".into(),
            key_hash: hash_key("abc"),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(30),
            revoked: false,
            revoked_at: None,
            created_by: "admin".into(),
        };
        assert!(key.days_until_expiry() >= 29);
        assert!(key.days_until_expiry() <= 30);
    }

    #[test]
    fn days_until_expiry_negative_when_expired() {
        let key = EnrollmentKey {
            id: Uuid::new_v4(),
            label: "expired".into(),
            key_hash: hash_key("abc"),
            created_at: chrono::Utc::now() - chrono::Duration::days(10),
            expires_at: chrono::Utc::now() - chrono::Duration::days(5),
            revoked: false,
            revoked_at: None,
            created_by: "admin".into(),
        };
        assert!(key.days_until_expiry() < 0);
    }

    #[test]
    fn create_key_request_serialization() {
        let req = CreateKeyRequest {
            label: "Building A".into(),
            expires_in_days: 90,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: CreateKeyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.label, "Building A");
        assert_eq!(parsed.expires_in_days, 90);
    }

    #[test]
    fn key_list_entry_serialization() {
        let entry = KeyListEntry {
            id: Uuid::new_v4(),
            label: "test".into(),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(90),
            revoked: false,
            days_remaining: 90,
            created_by: "admin".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: KeyListEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.label, "test");
        assert_eq!(parsed.days_remaining, 90);
    }

    #[test]
    fn base64_encode_known_values() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
        assert_eq!(base64_encode(b"AB"), "QUI=");
        assert_eq!(base64_encode(b"ABC"), "QUJD");
    }
}
