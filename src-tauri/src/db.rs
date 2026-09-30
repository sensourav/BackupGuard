use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

use crate::drives::Volume;
#[cfg(test)]
use crate::walker::WalkedFile;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub id: String,
    pub name: String,
    pub local_path: String,
    pub external_path: String,
    pub cloud_path: String,
    pub last_scanned: Option<String>,
    pub created_at: String,
    pub cloud_checked: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriveMount {
    pub drive_letter: String,
    pub label: String,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewFolder {
    pub name: String,
    pub local_path: String,
    pub external_path: String,
    pub cloud_path: String,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|err| err.to_string())?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.lock()?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|err| err.to_string())?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS watched_folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                local_path TEXT NOT NULL,
                external_path TEXT NOT NULL,
                cloud_path TEXT NOT NULL,
                last_scanned TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS drive_mounts (
                drive_letter TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('CONNECTED', 'UNMOUNTED')),
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS file_index (
                id TEXT PRIMARY KEY,
                folder_set_id TEXT NOT NULL,
                target TEXT NOT NULL CHECK (target IN ('local', 'external')),
                relative_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                last_modified TEXT NOT NULL,
                sha256_hash TEXT,
                quick_xor_hash TEXT,
                health_status TEXT,
                UNIQUE (folder_set_id, target, relative_path),
                FOREIGN KEY (folder_set_id) REFERENCES watched_folders(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS cloud_index (
                id TEXT PRIMARY KEY,
                folder_set_id TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                last_modified TEXT NOT NULL,
                quick_xor_hash TEXT,
                sha256_hash TEXT,
                UNIQUE (folder_set_id, relative_path),
                FOREIGN KEY (folder_set_id) REFERENCES watched_folders(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS oauth_session (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                account TEXT,
                expires_at TEXT NOT NULL,
                access_token BLOB NOT NULL,
                refresh_token BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                folder_set_id TEXT NOT NULL,
                scan_date TEXT NOT NULL,
                total_files INTEGER NOT NULL,
                healthy_count INTEGER NOT NULL,
                missing_count INTEGER NOT NULL,
                corrupt_count INTEGER NOT NULL,
                bitrot_count INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                bytes_processed INTEGER NOT NULL DEFAULT 0,
                summary TEXT,
                FOREIGN KEY (folder_set_id) REFERENCES watched_folders(id) ON DELETE CASCADE
            );
            ",
        )
        .map_err(|err| err.to_string())?;
        ensure_column(&conn, "file_index", "health_status", "TEXT")?;
        ensure_column(
            &conn,
            "audit_logs",
            "duration_ms",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "audit_logs",
            "bytes_processed",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&conn, "audit_logs", "summary", "TEXT")?;
        ensure_column(
            &conn,
            "watched_folders",
            "cloud_checked",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Ok(())
    }

    pub fn list_folders(&self) -> Result<Vec<WatchedFolder>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "
                SELECT id, name, local_path, external_path, cloud_path, last_scanned, created_at, cloud_checked
                FROM watched_folders
                ORDER BY name COLLATE NOCASE, id
                ",
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map([], map_folder)
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn add_folder(&self, folder: NewFolder) -> Result<WatchedFolder, String> {
        let name = required("name", &folder.name)?;
        let local_path = required("local path", &folder.local_path)?;
        let external_path = required("external path", &folder.external_path)?;
        let cloud_path = required("cloud path", &folder.cloud_path)?;
        let created = WatchedFolder {
            id: Uuid::new_v4().to_string(),
            name,
            local_path,
            external_path,
            cloud_path,
            last_scanned: None,
            created_at: now_iso(),
            cloud_checked: false,
        };
        let conn = self.lock()?;
        conn.execute(
            "
            INSERT INTO watched_folders (
                id, name, local_path, external_path, cloud_path, last_scanned, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)
            ",
            params![
                created.id,
                created.name,
                created.local_path,
                created.external_path,
                created.cloud_path,
                created.created_at,
            ],
        )
        .map_err(|err| err.to_string())?;
        Ok(created)
    }

    pub fn remove_folder(&self, id: &str) -> Result<(), String> {
        let conn = self.lock()?;
        let removed = conn
            .execute("DELETE FROM watched_folders WHERE id = ?1", params![id])
            .map_err(|err| err.to_string())?;
        if removed == 0 {
            return Err("watched folder was not found".to_string());
        }
        Ok(())
    }

    pub fn folder_by_id(&self, id: &str) -> Result<WatchedFolder, String> {
        let conn = self.lock()?;
        conn.query_row(
            "
            SELECT id, name, local_path, external_path, cloud_path, last_scanned, created_at, cloud_checked
            FROM watched_folders
            WHERE id = ?1
            ",
            params![id],
            map_folder,
        )
        .map_err(|_| "watched folder was not found".to_string())
    }

    pub fn list_drives(&self) -> Result<Vec<DriveMount>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "
                SELECT drive_letter, label, status, updated_at
                FROM drive_mounts
                ORDER BY drive_letter
                ",
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map([], map_drive)
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn apply_volumes(&self, present: &[Volume]) -> Result<bool, String> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|err| err.to_string())?;
        let existing = load_drives(&tx)?;
        let now = now_iso();
        let mut changed = false;
        let present_letters: HashMap<&str, &Volume> = present
            .iter()
            .map(|volume| (volume.drive_letter.as_str(), volume))
            .collect();

        for volume in present {
            let same = existing.get(&volume.drive_letter).is_some_and(|row| {
                row.status == "CONNECTED" && row.label == volume.label
            });
            if same {
                continue;
            }
            tx.execute(
                "
                INSERT INTO drive_mounts (drive_letter, label, status, updated_at)
                VALUES (?1, ?2, 'CONNECTED', ?3)
                ON CONFLICT(drive_letter) DO UPDATE SET
                    label = excluded.label,
                    status = 'CONNECTED',
                    updated_at = excluded.updated_at
                ",
                params![volume.drive_letter, volume.label, now],
            )
            .map_err(|err| err.to_string())?;
            changed = true;
        }

        for (letter, row) in &existing {
            if present_letters.contains_key(letter.as_str()) || row.status == "UNMOUNTED" {
                continue;
            }
            tx.execute(
                "
                UPDATE drive_mounts
                SET status = 'UNMOUNTED', updated_at = ?2
                WHERE drive_letter = ?1
                ",
                params![letter, now],
            )
            .map_err(|err| err.to_string())?;
            changed = true;
        }

        tx.commit().map_err(|err| err.to_string())?;
        Ok(changed)
    }

    #[cfg(test)]
    pub fn replace_file_index(
        &self,
        folder_set_id: &str,
        target: &str,
        files: &[WalkedFile],
    ) -> Result<(), String> {
        if target != "local" && target != "external" {
            return Err("file index target must be local or external".to_string());
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(|err| err.to_string())?;
        tx.execute(
            "DELETE FROM file_index WHERE folder_set_id = ?1 AND target = ?2",
            params![folder_set_id, target],
        )
        .map_err(|err| err.to_string())?;
        for file in files {
            tx.execute(
                "
                INSERT INTO file_index (
                    id, folder_set_id, target, relative_path, file_size, last_modified,
                    sha256_hash, quick_xor_hash
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)
                ",
                params![
                    Uuid::new_v4().to_string(),
                    folder_set_id,
                    target,
                    file.relative_path,
                    file.size_bytes as i64,
                    file.modified,
                ],
            )
            .map_err(|err| err.to_string())?;
        }
        tx.commit().map_err(|err| err.to_string())?;
        Ok(())
    }

    #[cfg(test)]
    pub fn hash_columns_are_null(&self, folder_set_id: &str) -> Result<bool, String> {
        let conn = self.lock()?;
        let count: i64 = conn
            .query_row(
                "
                SELECT COUNT(*) FROM file_index
                WHERE folder_set_id = ?1
                  AND (sha256_hash IS NOT NULL OR quick_xor_hash IS NOT NULL)
                ",
                params![folder_set_id],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        Ok(count == 0)
    }

    pub fn mark_scanned(&self, id: &str, scanned_at: &str) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE watched_folders SET last_scanned = ?2 WHERE id = ?1",
            params![id, scanned_at],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn index_map(
        &self,
        folder_set_id: &str,
        target: &str,
    ) -> Result<HashMap<String, IndexRow>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "
                SELECT relative_path, file_size, last_modified, sha256_hash, quick_xor_hash, health_status
                FROM file_index
                WHERE folder_set_id = ?1 AND target = ?2
                ",
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map(params![folder_set_id, target], map_index_row)
            .map_err(|err| err.to_string())?;
        let mut indexed = HashMap::new();
        for row in rows {
            let row = row.map_err(|err| err.to_string())?;
            indexed.insert(row.relative_path.clone(), row);
        }
        Ok(indexed)
    }

    pub fn upsert_hashed_row(
        &self,
        folder_set_id: &str,
        target: &str,
        relative_path: &str,
        size_bytes: u64,
        modified: &str,
        sha256: &str,
        quickxor: &str,
        health_status: &str,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "
            INSERT INTO file_index (
                id, folder_set_id, target, relative_path, file_size, last_modified,
                sha256_hash, quick_xor_hash, health_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(folder_set_id, target, relative_path) DO UPDATE SET
                file_size = excluded.file_size,
                last_modified = excluded.last_modified,
                sha256_hash = excluded.sha256_hash,
                quick_xor_hash = excluded.quick_xor_hash,
                health_status = excluded.health_status
            ",
            params![
                Uuid::new_v4().to_string(),
                folder_set_id,
                target,
                relative_path,
                size_bytes as i64,
                modified,
                sha256,
                quickxor,
                health_status,
            ],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn delete_index_path(
        &self,
        folder_set_id: &str,
        target: &str,
        relative_path: &str,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "
            DELETE FROM file_index
            WHERE folder_set_id = ?1 AND target = ?2 AND relative_path = ?3
            ",
            params![folder_set_id, target, relative_path],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn insert_audit_log(
        &self,
        folder_set_id: &str,
        scan_date: &str,
        total_files: i64,
        healthy_count: i64,
        missing_count: i64,
        corrupt_count: i64,
        bitrot_count: i64,
        duration_ms: i64,
        bytes_processed: i64,
        summary: &str,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "
            INSERT INTO audit_logs (
                id, folder_set_id, scan_date, total_files, healthy_count, missing_count,
                corrupt_count, bitrot_count, duration_ms, bytes_processed, summary
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                Uuid::new_v4().to_string(),
                folder_set_id,
                scan_date,
                total_files,
                healthy_count,
                missing_count,
                corrupt_count,
                bitrot_count,
                duration_ms,
                bytes_processed,
                summary,
            ],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn list_audit_entries(&self) -> Result<Vec<StoredAudit>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "
                SELECT audit_logs.id, audit_logs.scan_date, audit_logs.folder_set_id,
                       watched_folders.name, audit_logs.summary, audit_logs.healthy_count,
                       audit_logs.missing_count, audit_logs.corrupt_count, audit_logs.bitrot_count,
                       audit_logs.duration_ms, audit_logs.bytes_processed
                FROM audit_logs
                JOIN watched_folders ON watched_folders.id = audit_logs.folder_set_id
                ORDER BY audit_logs.scan_date DESC
                ",
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredAudit {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    set_id: row.get(2)?,
                    set_name: row.get(3)?,
                    summary: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    healthy: row.get(5)?,
                    missing: row.get(6)?,
                    corrupt: row.get(7)?,
                    bit_rot: row.get(8)?,
                    duration_ms: row.get(9)?,
                    bytes_processed: row.get(10)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn list_index_entries(&self) -> Result<Vec<StoredIndex>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "
                SELECT id, folder_set_id, target, relative_path, file_size, last_modified,
                       sha256_hash, quick_xor_hash, health_status
                FROM file_index
                ORDER BY relative_path
                ",
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredIndex {
                    id: row.get(0)?,
                    folder_set_id: row.get(1)?,
                    target: row.get(2)?,
                    relative_path: row.get(3)?,
                    file_size: row.get(4)?,
                    last_modified: row.get(5)?,
                    sha256_hash: row.get(6)?,
                    quick_xor_hash: row.get(7)?,
                    health_status: row.get(8)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn set_cloud_checked(&self, id: &str, checked: bool) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE watched_folders SET cloud_checked = ?2 WHERE id = ?1",
            params![id, if checked { 1 } else { 0 }],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn list_cloud_entries(&self) -> Result<Vec<StoredCloud>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "
                SELECT id, folder_set_id, relative_path, file_size, last_modified, quick_xor_hash
                FROM cloud_index
                ORDER BY relative_path
                ",
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredCloud {
                    id: row.get(0)?,
                    folder_set_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    file_size: row.get(3)?,
                    last_modified: row.get(4)?,
                    quick_xor_hash: row.get(5)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn list_cloud_for_folder(&self, folder_set_id: &str) -> Result<Vec<StoredCloud>, String> {
        Ok(self
            .list_cloud_entries()?
            .into_iter()
            .filter(|row| row.folder_set_id == folder_set_id)
            .collect())
    }

    pub fn delete_cloud_path(&self, folder_set_id: &str, relative_path: &str) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM cloud_index WHERE folder_set_id = ?1 AND relative_path = ?2",
            params![folder_set_id, relative_path],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn upsert_cloud(
        &self,
        folder_set_id: &str,
        relative_path: &str,
        size_bytes: u64,
        modified: &str,
        quickxor: Option<&str>,
        sha256: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "
            INSERT INTO cloud_index (
                id, folder_set_id, relative_path, file_size, last_modified, quick_xor_hash, sha256_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(folder_set_id, relative_path) DO UPDATE SET
                file_size = excluded.file_size,
                last_modified = excluded.last_modified,
                quick_xor_hash = excluded.quick_xor_hash,
                sha256_hash = excluded.sha256_hash
            ",
            params![
                Uuid::new_v4().to_string(),
                folder_set_id,
                relative_path,
                size_bytes as i64,
                modified,
                quickxor,
                sha256,
            ],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn save_oauth_session(
        &self,
        account: &str,
        expires_at: &str,
        access_token: &[u8],
        refresh_token: &[u8],
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "
            INSERT INTO oauth_session (id, account, expires_at, access_token, refresh_token)
            VALUES (1, ?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                account = excluded.account,
                expires_at = excluded.expires_at,
                access_token = excluded.access_token,
                refresh_token = excluded.refresh_token
            ",
            params![account, expires_at, access_token, refresh_token],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn oauth_session(&self) -> Result<Option<OauthSession>, String> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(
                "SELECT account, expires_at, access_token, refresh_token FROM oauth_session WHERE id = 1",
            )
            .map_err(|err| err.to_string())?;
        let mut rows = statement
            .query([])
            .map_err(|err| err.to_string())?;
        let Some(row) = rows.next().map_err(|err| err.to_string())? else {
            return Ok(None);
        };
        Ok(Some(OauthSession {
            account: row.get(0).map_err(|err| err.to_string())?,
            expires_at: row.get(1).map_err(|err| err.to_string())?,
            access_token: row.get(2).map_err(|err| err.to_string())?,
            refresh_token: row.get(3).map_err(|err| err.to_string())?,
        }))
    }

    pub fn clear_oauth_session(&self) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM oauth_session", [])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|err| err.to_string())
    }
}

fn required(field: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{field} is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[derive(Debug, Clone)]
pub struct IndexRow {
    pub relative_path: String,
    pub file_size: i64,
    pub last_modified: String,
    pub sha256_hash: Option<String>,
    pub quick_xor_hash: Option<String>,
    #[allow(dead_code)]
    pub health_status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAudit {
    pub id: String,
    pub timestamp: String,
    pub set_id: String,
    pub set_name: String,
    pub summary: String,
    pub healthy: i64,
    pub missing: i64,
    pub corrupt: i64,
    pub bit_rot: i64,
    pub duration_ms: i64,
    pub bytes_processed: i64,
}

#[derive(Debug, Clone)]
pub struct StoredIndex {
    pub id: String,
    pub folder_set_id: String,
    pub target: String,
    pub relative_path: String,
    pub file_size: i64,
    pub last_modified: String,
    pub sha256_hash: Option<String>,
    pub quick_xor_hash: Option<String>,
    pub health_status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredCloud {
    pub id: String,
    pub folder_set_id: String,
    pub relative_path: String,
    pub file_size: i64,
    pub last_modified: String,
    pub quick_xor_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OauthSession {
    pub account: String,
    pub expires_at: String,
    pub access_token: Vec<u8>,
    pub refresh_token: Vec<u8>,
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<(), String> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|err| err.to_string())?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| err.to_string())?;
    for name in names {
        if name.map_err(|err| err.to_string())? == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn map_index_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexRow> {
    Ok(IndexRow {
        relative_path: row.get(0)?,
        file_size: row.get(1)?,
        last_modified: row.get(2)?,
        sha256_hash: row.get(3)?,
        quick_xor_hash: row.get(4)?,
        health_status: row.get(5)?,
    })
}

fn map_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchedFolder> {
    Ok(WatchedFolder {
        id: row.get(0)?,
        name: row.get(1)?,
        local_path: row.get(2)?,
        external_path: row.get(3)?,
        cloud_path: row.get(4)?,
        last_scanned: row.get(5)?,
        created_at: row.get(6)?,
        cloud_checked: row.get::<_, i64>(7)? != 0,
    })
}

fn map_drive(row: &rusqlite::Row<'_>) -> rusqlite::Result<DriveMount> {
    Ok(DriveMount {
        drive_letter: row.get(0)?,
        label: row.get(1)?,
        status: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

fn load_drives(conn: &Connection) -> Result<HashMap<String, DriveMount>, String> {
    let mut statement = conn
        .prepare("SELECT drive_letter, label, status, updated_at FROM drive_mounts")
        .map_err(|err| err.to_string())?;
    let rows = statement
        .query_map([], map_drive)
        .map_err(|err| err.to_string())?;
    let mut drives = HashMap::new();
    for row in rows {
        let drive = row.map_err(|err| err.to_string())?;
        drives.insert(drive.drive_letter.clone(), drive);
    }
    Ok(drives)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn folder_delete_removes_the_row_and_leaves_files() {
        let root = crate::walker::fixture_root("db");
        fs::create_dir_all(&root).unwrap();
        let file_path = root.join("keep.txt");
        fs::write(&file_path, b"keep").unwrap();

        let db = Database::open_in_memory().unwrap();
        let folder = db
            .add_folder(NewFolder {
                name: "DJI".to_string(),
                local_path: root.display().to_string(),
                external_path: "E:/Backups/DJI/".to_string(),
                cloud_path: "/OneDrive/Backups/DJI/".to_string(),
            })
            .unwrap();
        db.remove_folder(&folder.id).unwrap();

        assert!(db.list_folders().unwrap().is_empty());
        assert_eq!(fs::read(&file_path).unwrap(), b"keep");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_external_letter_becomes_unmounted() {
        let db = Database::open_in_memory().unwrap();
        db.apply_volumes(&[
            Volume {
                drive_letter: "C:".to_string(),
                label: "OS".to_string(),
            },
            Volume {
                drive_letter: "E:".to_string(),
                label: "Backup".to_string(),
            },
        ])
        .unwrap();
        db.apply_volumes(&[Volume {
            drive_letter: "C:".to_string(),
            label: "OS".to_string(),
        }])
        .unwrap();

        let mounts = db.list_drives().unwrap();
        let external = mounts.iter().find(|mount| mount.drive_letter == "E:").unwrap();
        assert_eq!(external.status, "UNMOUNTED");
        let local = mounts.iter().find(|mount| mount.drive_letter == "C:").unwrap();
        assert_eq!(local.status, "CONNECTED");
    }

    #[test]
    fn file_index_keeps_hash_columns_null() {
        let db = Database::open_in_memory().unwrap();
        let folder = db
            .add_folder(NewFolder {
                name: "DJI".to_string(),
                local_path: "C:/Backups/DJI/".to_string(),
                external_path: "E:/Backups/DJI/".to_string(),
                cloud_path: "/OneDrive/Backups/DJI/".to_string(),
            })
            .unwrap();
        db.replace_file_index(
            &folder.id,
            "local",
            &[WalkedFile {
                name: "clip.txt".to_string(),
                relative_path: "2026-09-16/clip.txt".to_string(),
                size_bytes: 3,
                modified: "2026-09-16T00:00:00Z".to_string(),
            }],
        )
        .unwrap();
        assert!(db.hash_columns_are_null(&folder.id).unwrap());
    }
}
