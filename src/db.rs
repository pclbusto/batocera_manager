use rusqlite::{params, Connection, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub use crate::domain::sync::{SyncFileRecord, SyncStatus, SyncSummary};

pub fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".local/share/batocera_manager");
    let _ = fs::create_dir_all(&dir);
    dir.join("sync.db")
}

pub fn init_db(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_files (
            relative_path TEXT PRIMARY KEY,
            category TEXT NOT NULL,
            local_size INTEGER,
            local_mtime INTEGER,
            local_hash TEXT,
            remote_size INTEGER,
            remote_mtime INTEGER,
            remote_hash TEXT,
            status TEXT NOT NULL,
            last_sync INTEGER
        );
        CREATE TABLE IF NOT EXISTS system_settings (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            local_folder TEXT NOT NULL,
            enabled INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_category ON sync_files (category);
        CREATE INDEX IF NOT EXISTS idx_status ON sync_files (status);",
    )?;

    Ok(conn)
}

pub fn save_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn load_setting(conn: &Connection, key: &str) -> Option<String> {
    let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1").ok()?;
    stmt.query_row(params![key], |row| row.get(0)).ok()
}

pub fn save_system_setting(
    conn: &Connection,
    id: &str,
    name: &str,
    local_folder: &str,
    enabled: bool,
) -> Result<()> {
    conn.execute(
        "INSERT INTO system_settings (id, name, local_folder, enabled)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            local_folder = excluded.local_folder,
            enabled = excluded.enabled",
        params![id, name, local_folder, if enabled { 1 } else { 0 }],
    )?;
    Ok(())
}

pub fn load_system_settings(conn: &Connection) -> Result<Vec<(String, String, String, bool)>> {
    let mut stmt = conn.prepare("SELECT id, name, local_folder, enabled FROM system_settings")?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let folder: String = row.get(2)?;
        let enabled_int: i32 = row.get(3)?;
        Ok((id, name, folder, enabled_int == 1))
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn upsert_file_record(conn: &Connection, record: &SyncFileRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_files (
            relative_path, category, local_size, local_mtime, local_hash,
            remote_size, remote_mtime, remote_hash, status, last_sync
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(relative_path) DO UPDATE SET
            category = excluded.category,
            local_size = excluded.local_size,
            local_mtime = excluded.local_mtime,
            local_hash = excluded.local_hash,
            remote_size = excluded.remote_size,
            remote_mtime = excluded.remote_mtime,
            remote_hash = excluded.remote_hash,
            status = excluded.status,
            last_sync = excluded.last_sync",
        params![
            record.relative_path,
            record.category,
            record.local_size.map(|v| v as i64),
            record.local_mtime.map(|v| v as i64),
            record.local_hash,
            record.remote_size.map(|v| v as i64),
            record.remote_mtime.map(|v| v as i64),
            record.remote_hash,
            record.status.as_str(),
            record.last_sync.map(|v| v as i64),
        ],
    )?;
    Ok(())
}

pub fn get_all_records(
    conn: &Connection,
    category_filter: Option<&str>,
) -> Result<Vec<SyncFileRecord>> {
    let mut query = "SELECT relative_path, category, local_size, local_mtime, local_hash,
                            remote_size, remote_mtime, remote_hash, status, last_sync
                     FROM sync_files".to_string();

    if let Some(cat) = category_filter {
        if cat != "all" {
            query.push_str(&format!(" WHERE category = '{cat}'"));
        }
    }
    query.push_str(" ORDER BY relative_path ASC");

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        let status_str: String = row.get(8)?;
        Ok(SyncFileRecord {
            relative_path: row.get(0)?,
            category: row.get(1)?,
            local_size: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
            local_mtime: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
            local_hash: row.get(4)?,
            remote_size: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
            remote_mtime: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
            remote_hash: row.get(7)?,
            status: SyncStatus::from_str(&status_str),
            last_sync: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_sync_summary(conn: &Connection) -> Result<SyncSummary> {
    let mut stmt = conn.prepare("SELECT status, count(*) FROM sync_files GROUP BY status")?;
    let rows = stmt.query_map([], |row| {
        let status_str: String = row.get(0)?;
        let count: usize = row.get(1)?;
        Ok((status_str, count))
    })?;

    let mut summary = SyncSummary::default();
    for r in rows {
        let (status_str, count) = r?;
        summary.total += count;
        match SyncStatus::from_str(&status_str) {
            SyncStatus::InSync => summary.in_sync += count,
            SyncStatus::OnlyLocal => summary.only_local += count,
            SyncStatus::OnlyRemote => summary.only_remote += count,
            SyncStatus::LocalNewer => summary.local_newer += count,
            SyncStatus::RemoteNewer => summary.remote_newer += count,
            SyncStatus::Conflict => {}
        }
    }

    Ok(summary)
}

#[allow(dead_code)]
pub fn clear_all_records(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM sync_files", [])?;
    Ok(())
}
