use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use imaging_common::config::OsFamily;
use imaging_common::driver::DriverPack;
use imaging_common::security::EnrollmentKey;
use imaging_common::sequence::TaskSequence;
use imaging_common::types::{AuditEntry, AuditLevel, DeployJob, ImageRecord};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    os_family TEXT NOT NULL,
    captured_from TEXT,
    build_date TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,
    wim_index INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS deploy_jobs (
    id TEXT PRIMARY KEY,
    machine_name TEXT NOT NULL,
    image_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    progress_pct INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    started_by TEXT NOT NULL,
    message TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    detail TEXT,
    level TEXT NOT NULL DEFAULT 'info'
);

CREATE TABLE IF NOT EXISTS sequences (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS driver_packs (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS enrollment_keys (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0,
    revoked_at TEXT,
    created_by TEXT NOT NULL
);
"#;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("set pragmas")?;

        conn.execute_batch(SCHEMA).context("create schema")?;

        // Additive migrations for columns added post-ship. SQLite has no
        // ADD COLUMN IF NOT EXISTS, so check pragma_table_info first.
        add_column_if_missing(&conn, "images", "wim_index",
            "INTEGER NOT NULL DEFAULT 1")?;

        tracing::info!("Database opened at {}", path.display());
        Ok(Self { conn })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory db")?;
        conn.execute_batch(SCHEMA).context("create schema")?;
        add_column_if_missing(&conn, "images", "wim_index",
            "INTEGER NOT NULL DEFAULT 1")?;
        Ok(Self { conn })
    }

    // --- Images ---

    pub fn insert_image(&self, img: &ImageRecord) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO images (id, name, os_family, captured_from, build_date, size_bytes, sha256, created_at, wim_index) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                img.id.to_string(),
                img.name,
                format!("{:?}", img.os_family).to_lowercase(),
                img.captured_from,
                img.build_date.to_string(),
                img.size_bytes as i64,
                img.sha256,
                img.created_at.to_rfc3339(),
                img.wim_index as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_images(&self) -> Result<Vec<ImageRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, os_family, captured_from, build_date, size_bytes, sha256, created_at, wim_index FROM images ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let os_str: String = row.get(2)?;
            let date_str: String = row.get(4)?;
            let created_str: String = row.get(7)?;
            Ok(ImageRecord {
                id: id_str.parse().unwrap_or_default(),
                name: row.get(1)?,
                os_family: parse_os_family(&os_str),
                captured_from: row.get(3)?,
                build_date: date_str.parse().unwrap_or_default(),
                size_bytes: row.get::<_, i64>(5)? as u64,
                sha256: row.get(6)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                wim_index: row.get::<_, i64>(8)? as u32,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().context("read images")
    }

    pub fn get_image(&self, id: &Uuid) -> Result<Option<ImageRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, os_family, captured_from, build_date, size_bytes, sha256, created_at, wim_index FROM images WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(params![id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let os_str: String = row.get(2)?;
            let date_str: String = row.get(4)?;
            let created_str: String = row.get(7)?;
            Ok(ImageRecord {
                id: id_str.parse().unwrap_or_default(),
                name: row.get(1)?,
                os_family: parse_os_family(&os_str),
                captured_from: row.get(3)?,
                build_date: date_str.parse().unwrap_or_default(),
                size_bytes: row.get::<_, i64>(5)? as u64,
                sha256: row.get(6)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                wim_index: row.get::<_, i64>(8)? as u32,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // --- Deploy Jobs ---

    pub fn upsert_deploy_job(&self, job: &DeployJob, message: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO deploy_jobs (id, machine_name, image_id, status, progress_pct, started_at, started_by, message) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET status=?4, progress_pct=?5, message=?8",
            params![
                job.id.to_string(),
                job.machine_name,
                job.image_id.to_string(),
                format!("{:?}", job.status).to_lowercase(),
                job.progress_pct as i32,
                job.started_at.to_rfc3339(),
                job.started_by,
                message,
            ],
        )?;
        Ok(())
    }

    pub fn list_deploy_jobs(&self) -> Result<Vec<DeployJobRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, machine_name, image_id, status, progress_pct, started_at, started_by, message FROM deploy_jobs ORDER BY started_at DESC LIMIT 100"
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let status_str: String = row.get(3)?;
            let started_str: String = row.get(5)?;
            Ok(DeployJobRow {
                id: id_str,
                machine_name: row.get(1)?,
                image_id: row.get(2)?,
                status: status_str,
                progress_pct: row.get::<_, i32>(4)? as u8,
                started_at: started_str,
                started_by: row.get(6)?,
                message: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().context("read deploy jobs")
    }

    // --- Audit Log ---

    pub fn insert_audit(&self, entry: &AuditEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit_log (id, timestamp, actor, action, target, detail, level) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                entry.id.to_string(),
                entry.timestamp.to_rfc3339(),
                entry.actor,
                entry.action,
                entry.target,
                entry.detail,
                format!("{:?}", entry.level).to_lowercase(),
            ],
        )?;
        Ok(())
    }

    pub fn query_audit(&self, search: Option<&str>, level: Option<&str>, limit: u32) -> Result<Vec<AuditEntry>> {
        let query = if search.is_some() || level.is_some() {
            let mut conditions = Vec::new();
            if search.is_some() {
                conditions.push("(action LIKE '%' || ?1 || '%' OR target LIKE '%' || ?1 || '%' OR actor LIKE '%' || ?1 || '%' OR detail LIKE '%' || ?1 || '%')");
            }
            if level.is_some() {
                conditions.push("level = ?2");
            }
            format!(
                "SELECT id, timestamp, actor, action, target, detail, level FROM audit_log WHERE {} ORDER BY timestamp DESC LIMIT ?3",
                conditions.join(" AND ")
            )
        } else {
            "SELECT id, timestamp, actor, action, target, detail, level FROM audit_log ORDER BY timestamp DESC LIMIT ?3".into()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let search_val = search.unwrap_or("");
        let level_val = level.unwrap_or("");
        let limit_val = limit as i64;

        let rows = stmt.query_map(params![search_val, level_val, limit_val], |row| {
            let id_str: String = row.get(0)?;
            let ts_str: String = row.get(1)?;
            let level_str: String = row.get(6)?;
            Ok(AuditEntry {
                id: id_str.parse().unwrap_or_default(),
                timestamp: chrono::DateTime::parse_from_rfc3339(&ts_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                actor: row.get(2)?,
                action: row.get(3)?,
                target: row.get(4)?,
                detail: row.get(5)?,
                level: parse_audit_level(&level_str),
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().context("read audit log")
    }

    // --- Sequences ---

    pub fn upsert_sequence(&self, seq: &TaskSequence) -> Result<()> {
        let data = serde_json::to_string(seq).context("serialize sequence")?;
        self.conn.execute(
            "INSERT INTO sequences (id, data, created_at, updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET data=?2, updated_at=?4",
            params![
                seq.id.to_string(),
                data,
                seq.created_at.to_rfc3339(),
                seq.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_sequences(&self) -> Result<Vec<TaskSequence>> {
        let mut stmt = self.conn.prepare("SELECT data FROM sequences ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut result = Vec::new();
        for row in rows {
            let data = row?;
            if let Ok(seq) = serde_json::from_str::<TaskSequence>(&data) {
                result.push(seq);
            }
        }
        Ok(result)
    }

    pub fn delete_sequence(&self, id: &Uuid) -> Result<bool> {
        let count = self.conn.execute("DELETE FROM sequences WHERE id = ?1", params![id.to_string()])?;
        Ok(count > 0)
    }

    // --- Driver Packs ---

    pub fn upsert_driver_pack(&self, pack: &DriverPack) -> Result<()> {
        let data = serde_json::to_string(pack).context("serialize driver pack")?;
        self.conn.execute(
            "INSERT INTO driver_packs (id, data, created_at) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET data=?2",
            params![pack.id.to_string(), data, pack.created_at.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn list_driver_packs(&self) -> Result<Vec<DriverPack>> {
        let mut stmt = self.conn.prepare("SELECT data FROM driver_packs ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut result = Vec::new();
        for row in rows {
            let data = row?;
            if let Ok(pack) = serde_json::from_str::<DriverPack>(&data) {
                result.push(pack);
            }
        }
        Ok(result)
    }

    pub fn delete_driver_pack(&self, id: &Uuid) -> Result<bool> {
        let count = self.conn.execute("DELETE FROM driver_packs WHERE id = ?1", params![id.to_string()])?;
        Ok(count > 0)
    }

    // --- Enrollment Keys ---

    pub fn insert_enrollment_key(&self, key: &EnrollmentKey) -> Result<()> {
        self.conn.execute(
            "INSERT INTO enrollment_keys (id, label, key_hash, created_at, expires_at, revoked, revoked_at, created_by) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                key.id.to_string(),
                key.label,
                key.key_hash,
                key.created_at.to_rfc3339(),
                key.expires_at.to_rfc3339(),
                key.revoked as i32,
                key.revoked_at.map(|d| d.to_rfc3339()),
                key.created_by,
            ],
        )?;
        Ok(())
    }

    pub fn list_enrollment_keys(&self) -> Result<Vec<EnrollmentKey>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, label, key_hash, created_at, expires_at, revoked, revoked_at, created_by FROM enrollment_keys ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let created_str: String = row.get(3)?;
            let expires_str: String = row.get(4)?;
            let revoked_int: i32 = row.get(5)?;
            let revoked_at_str: Option<String> = row.get(6)?;
            Ok(EnrollmentKey {
                id: id_str.parse().unwrap_or_default(),
                label: row.get(1)?,
                key_hash: row.get(2)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                expires_at: chrono::DateTime::parse_from_rfc3339(&expires_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                revoked: revoked_int != 0,
                revoked_at: revoked_at_str.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|d| d.with_timezone(&chrono::Utc))
                        .ok()
                }),
                created_by: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().context("read enrollment keys")
    }

    pub fn validate_enrollment_key(&self, key_hash: &str) -> Result<Option<EnrollmentKey>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, label, key_hash, created_at, expires_at, revoked, revoked_at, created_by FROM enrollment_keys WHERE key_hash = ?1"
        )?;

        let mut rows = stmt.query_map(params![key_hash], |row| {
            let id_str: String = row.get(0)?;
            let created_str: String = row.get(3)?;
            let expires_str: String = row.get(4)?;
            let revoked_int: i32 = row.get(5)?;
            let revoked_at_str: Option<String> = row.get(6)?;
            Ok(EnrollmentKey {
                id: id_str.parse().unwrap_or_default(),
                label: row.get(1)?,
                key_hash: row.get(2)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                expires_at: chrono::DateTime::parse_from_rfc3339(&expires_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                revoked: revoked_int != 0,
                revoked_at: revoked_at_str.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|d| d.with_timezone(&chrono::Utc))
                        .ok()
                }),
                created_by: row.get(7)?,
            })
        })?;

        match rows.next() {
            Some(Ok(key)) => Ok(Some(key)),
            _ => Ok(None),
        }
    }

    pub fn revoke_enrollment_key(&self, id: &Uuid) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "UPDATE enrollment_keys SET revoked = 1, revoked_at = ?2 WHERE id = ?1 AND revoked = 0",
            params![id.to_string(), now],
        )?;
        Ok(count > 0)
    }

    pub fn delete_enrollment_key(&self, id: &Uuid) -> Result<bool> {
        let count = self.conn.execute(
            "DELETE FROM enrollment_keys WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(count > 0)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeployJobRow {
    pub id: String,
    pub machine_name: String,
    pub image_id: String,
    pub status: String,
    pub progress_pct: u8,
    pub started_at: String,
    pub started_by: String,
    pub message: String,
}

pub fn audit(db: &Database, actor: &str, action: &str, target: &str, detail: Option<&str>, level: AuditLevel) {
    let entry = AuditEntry {
        id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        actor: actor.into(),
        action: action.into(),
        target: target.into(),
        detail: detail.map(String::from),
        level,
    };
    if let Err(e) = db.insert_audit(&entry) {
        tracing::error!("Failed to write audit entry: {e}");
    }
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    column_def: &str,
) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
        params![table, column],
        |row| row.get(0),
    ).with_context(|| format!("pragma_table_info({table})"))?;
    if count == 0 {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {column_def}"),
            [],
        ).with_context(|| format!("ALTER TABLE {table} ADD COLUMN {column}"))?;
        tracing::info!("Migrated {table}: added column `{column}`");
    }
    Ok(())
}

fn parse_os_family(s: &str) -> OsFamily {
    match s {
        "win10" => OsFamily::Win10,
        _ => OsFamily::Win11,
    }
}

fn parse_audit_level(s: &str) -> AuditLevel {
    match s {
        "warning" => AuditLevel::Warning,
        "error" => AuditLevel::Error,
        _ => AuditLevel::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use imaging_common::security;
    use imaging_common::types::DeployStatus;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn sample_image() -> ImageRecord {
        ImageRecord {
            id: Uuid::new_v4(),
            name: "Win11-Test".into(),
            os_family: OsFamily::Win11,
            captured_from: Some("TestPC".into()),
            build_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            size_bytes: 1_000_000,
            sha256: "abc123".into(),
            created_at: chrono::Utc::now(),
            wim_index: 1,
        }
    }

    fn sample_key(label: &str, plaintext: &str) -> EnrollmentKey {
        EnrollmentKey {
            id: Uuid::new_v4(),
            label: label.into(),
            key_hash: security::hash_key(plaintext),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(90),
            revoked: false,
            revoked_at: None,
            created_by: "test".into(),
        }
    }

    // --- Image CRUD ---

    #[test]
    fn insert_and_list_images() {
        let db = test_db();
        let img = sample_image();
        db.insert_image(&img).unwrap();

        let images = db.list_images().unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].name, "Win11-Test");
    }

    #[test]
    fn list_images_empty() {
        let db = test_db();
        let images = db.list_images().unwrap();
        assert!(images.is_empty());
    }

    // --- Deploy Jobs ---

    #[test]
    fn upsert_and_list_deploy_jobs() {
        let db = test_db();
        let job = DeployJob {
            id: Uuid::new_v4(),
            machine_name: "PC-001".into(),
            image_id: Uuid::new_v4(),
            status: DeployStatus::Applying,
            progress_pct: 50,
            started_at: chrono::Utc::now(),
            started_by: "agent".into(),
        };
        db.upsert_deploy_job(&job, "Applying image").unwrap();

        let jobs = db.list_deploy_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].machine_name, "PC-001");
        assert_eq!(jobs[0].progress_pct, 50);
    }

    #[test]
    fn upsert_deploy_job_updates() {
        let db = test_db();
        let id = Uuid::new_v4();
        let job = DeployJob {
            id,
            machine_name: "PC-002".into(),
            image_id: Uuid::new_v4(),
            status: DeployStatus::Pending,
            progress_pct: 0,
            started_at: chrono::Utc::now(),
            started_by: "agent".into(),
        };
        db.upsert_deploy_job(&job, "Starting").unwrap();

        let updated = DeployJob { progress_pct: 75, status: DeployStatus::Applying, ..job };
        db.upsert_deploy_job(&updated, "75% done").unwrap();

        let jobs = db.list_deploy_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].progress_pct, 75);
    }

    // --- Audit Log ---

    #[test]
    fn insert_and_query_audit() {
        let db = test_db();
        audit(&db, "admin", "test_action", "target1", Some("detail"), AuditLevel::Info);

        let entries = db.query_audit(None, None, 100).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].actor, "admin");
        assert_eq!(entries[0].action, "test_action");
    }

    #[test]
    fn query_audit_by_search() {
        let db = test_db();
        audit(&db, "admin", "deploy", "PC-001", None, AuditLevel::Info);
        audit(&db, "agent", "auth", "PC-002", None, AuditLevel::Info);

        let entries = db.query_audit(Some("deploy"), None, 100).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "deploy");
    }

    #[test]
    fn query_audit_by_level() {
        let db = test_db();
        audit(&db, "admin", "action1", "t1", None, AuditLevel::Info);
        audit(&db, "admin", "action2", "t2", None, AuditLevel::Warning);

        let entries = db.query_audit(None, Some("warning"), 100).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "action2");
    }

    // --- Enrollment Keys ---

    #[test]
    fn insert_and_list_enrollment_keys() {
        let db = test_db();
        let key = sample_key("Test Key", "secret123");
        db.insert_enrollment_key(&key).unwrap();

        let keys = db.list_enrollment_keys().unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].label, "Test Key");
        assert!(!keys[0].revoked);
    }

    #[test]
    fn validate_enrollment_key_by_hash() {
        let db = test_db();
        let plaintext = "my-secret-key";
        let key = sample_key("Lookup Test", plaintext);
        db.insert_enrollment_key(&key).unwrap();

        let hash = security::hash_key(plaintext);
        let found = db.validate_enrollment_key(&hash).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "Lookup Test");
    }

    #[test]
    fn validate_enrollment_key_unknown_hash() {
        let db = test_db();
        let key = sample_key("Test", "real-key");
        db.insert_enrollment_key(&key).unwrap();

        let found = db.validate_enrollment_key(&security::hash_key("wrong-key")).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn revoke_enrollment_key() {
        let db = test_db();
        let key = sample_key("Revoke Test", "key123");
        let id = key.id;
        db.insert_enrollment_key(&key).unwrap();

        assert!(db.revoke_enrollment_key(&id).unwrap());

        let keys = db.list_enrollment_keys().unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].revoked);
        assert!(keys[0].revoked_at.is_some());
    }

    #[test]
    fn revoke_already_revoked_returns_false() {
        let db = test_db();
        let key = sample_key("Double Revoke", "key456");
        let id = key.id;
        db.insert_enrollment_key(&key).unwrap();

        assert!(db.revoke_enrollment_key(&id).unwrap());
        assert!(!db.revoke_enrollment_key(&id).unwrap());
    }

    #[test]
    fn delete_enrollment_key() {
        let db = test_db();
        let key = sample_key("Delete Test", "key789");
        let id = key.id;
        db.insert_enrollment_key(&key).unwrap();

        assert!(db.delete_enrollment_key(&id).unwrap());
        assert!(db.list_enrollment_keys().unwrap().is_empty());
    }

    #[test]
    fn delete_nonexistent_key_returns_false() {
        let db = test_db();
        assert!(!db.delete_enrollment_key(&Uuid::new_v4()).unwrap());
    }

    #[test]
    fn multiple_keys_listed_newest_first() {
        let db = test_db();
        let mut k1 = sample_key("First", "aaa");
        k1.created_at = chrono::Utc::now() - chrono::Duration::hours(2);
        let mut k2 = sample_key("Second", "bbb");
        k2.created_at = chrono::Utc::now();

        db.insert_enrollment_key(&k1).unwrap();
        db.insert_enrollment_key(&k2).unwrap();

        let keys = db.list_enrollment_keys().unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].label, "Second");
        assert_eq!(keys[1].label, "First");
    }
}
