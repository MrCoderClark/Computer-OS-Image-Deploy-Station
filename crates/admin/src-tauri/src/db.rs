use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;

use imaging_common::driver::DriverPack;
use imaging_common::sequence::TaskSequence;

const SCHEMA: &str = r#"
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

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub struct AdminDb {
    conn: Connection,
}

impl AdminDb {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open admin database at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .context("set pragmas")?;
        conn.execute_batch(SCHEMA).context("create schema")?;

        tracing::info!("Admin database opened at {}", path.display());
        Ok(Self { conn })
    }

    // --- Sequences ---

    pub fn upsert_sequence(&self, seq: &TaskSequence) -> Result<()> {
        let data = serde_json::to_string(seq).context("serialize sequence")?;
        self.conn.execute(
            "INSERT INTO sequences (id, data, created_at, updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET data=?2, updated_at=?4",
            params![seq.id.to_string(), data, seq.created_at.to_rfc3339(), seq.updated_at.to_rfc3339()],
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

    // --- Settings ---

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
            params![key, value],
        )?;
        Ok(())
    }
}
