mod audit;
mod db;
mod drives;
mod graph;
mod hash;
mod oauth;
mod quickxor;
mod walker;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{Emitter, Manager};

use audit::{parse_scope, run_audit, AuditRequest, CloudAudit, CloudView};
use db::{Database, DriveMount, NewFolder, StoredCloud, StoredIndex, WatchedFolder};
use drives::{letter_is_connected, list_connected_volumes};
use oauth::AuthError;

struct AppState {
    db: Database,
    client_config_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetCell {
    present: bool,
    size_bytes: Option<u64>,
    mtime: Option<String>,
    hash: Option<String>,
    unmounted: bool,
    offline: bool,
    unscanned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscrepancyDto {
    id: String,
    set_id: String,
    relative_path: String,
    status: String,
    local: TargetCell,
    external: TargetCell,
    cloud: TargetCell,
    note: Option<String>,
}

#[tauri::command]
fn list_watched_folders(state: tauri::State<'_, AppState>) -> Result<Vec<WatchedFolder>, String> {
    state.db.list_folders()
}

#[tauri::command]
fn add_watched_folder(
    state: tauri::State<'_, AppState>,
    name: String,
    local_path: String,
    external_path: String,
    cloud_path: String,
) -> Result<WatchedFolder, String> {
    state.db.add_folder(NewFolder {
        name,
        local_path,
        external_path,
        cloud_path,
    })
}

#[tauri::command]
fn remove_watched_folder(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.remove_folder(&id)
}

#[tauri::command]
fn list_drive_mounts(state: tauri::State<'_, AppState>) -> Result<Vec<DriveMount>, String> {
    state.db.list_drives()
}

#[tauri::command]
fn audit_folder_set(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    scope: String,
    relative_path: Option<String>,
) -> Result<audit::AuditReport, String> {
    let folder = state.db.folder_by_id(&id)?;
    let parsed = parse_scope(&scope, relative_path.as_deref())?;
    let volumes = list_connected_volumes().unwrap_or_default();
    let cloud = match oauth::ensure_access_token(&state.db) {
        Ok(token) => CloudAudit::Token(token),
        Err(AuthError::SignedOut) => CloudAudit::Skipped,
        Err(AuthError::Expired) => CloudAudit::Failed("OneDrive token expired".to_string()),
        Err(AuthError::Message(message)) => CloudAudit::Failed(message),
    };
    let request = AuditRequest {
        folder_id: folder.id,
        set_name: folder.name,
        local_path: folder.local_path.clone(),
        external_path: folder.external_path.clone(),
        cloud_path: folder.cloud_path.clone(),
        scope: parsed,
        local_connected: letter_is_connected(&folder.local_path, &volumes),
        external_connected: letter_is_connected(&folder.external_path, &volumes),
        cloud,
    };
    run_audit(&state.db, &request, |progress| {
        let _ = app.emit("audit-progress", progress);
    })
}

#[tauri::command]
fn list_discrepancies(state: tauri::State<'_, AppState>) -> Result<Vec<DiscrepancyDto>, String> {
    let folders = state.db.list_folders()?;
    let volumes = list_connected_volumes().unwrap_or_default();
    let rows = state.db.list_index_entries()?;
    let cloud_rows = state.db.list_cloud_entries()?;
    let cloud_connected = matches!(oauth::link_state(&state.db)?, oauth::LinkState::Connected);
    Ok(build_discrepancies(
        &folders,
        &volumes,
        &rows,
        &cloud_rows,
        cloud_connected,
    ))
}

#[tauri::command]
fn onedrive_status(state: tauri::State<'_, AppState>) -> Result<oauth::OneDriveStatus, String> {
    oauth::status(&state.db)
}

#[tauri::command]
fn onedrive_connect(state: tauri::State<'_, AppState>) -> Result<oauth::OneDriveStatus, String> {
    oauth::connect(&state.db, &state.client_config_paths)
}

#[tauri::command]
fn onedrive_disconnect(state: tauri::State<'_, AppState>) -> Result<oauth::OneDriveStatus, String> {
    oauth::disconnect(&state.db)
}

#[tauri::command]
fn list_audit_logs(state: tauri::State<'_, AppState>) -> Result<Vec<db::StoredAudit>, String> {
    state.db.list_audit_entries()
}

struct IndexedPaths {
    local: Option<StoredIndex>,
    external: Option<StoredIndex>,
    cloud: Option<StoredCloud>,
}

fn build_discrepancies(
    folders: &[WatchedFolder],
    volumes: &[drives::Volume],
    rows: &[StoredIndex],
    cloud_rows: &[StoredCloud],
    cloud_connected: bool,
) -> Vec<DiscrepancyDto> {
    use std::collections::BTreeMap;

    let mut grouped: BTreeMap<(String, String), IndexedPaths> = BTreeMap::new();
    for row in rows {
        let entry = grouped
            .entry((row.folder_set_id.clone(), row.relative_path.clone()))
            .or_insert(IndexedPaths {
                local: None,
                external: None,
                cloud: None,
            });
        if row.target == "local" {
            entry.local = Some(row.clone());
        } else if row.target == "external" {
            entry.external = Some(row.clone());
        }
    }

    for row in cloud_rows {
        let folder = folders.iter().find(|folder| folder.id == row.folder_set_id);
        let cloud_active = cloud_connected && folder.is_some_and(|folder| folder.cloud_checked);
        if !cloud_active {
            continue;
        }
        let entry = grouped
            .entry((row.folder_set_id.clone(), row.relative_path.clone()))
            .or_insert(IndexedPaths {
                local: None,
                external: None,
                cloud: None,
            });
        entry.cloud = Some(row.clone());
    }

    grouped
        .into_iter()
        .map(|((set_id, relative_path), group)| {
            let folder = folders.iter().find(|folder| folder.id == set_id);
            let external_unmounted = folder
                .is_some_and(|folder| !letter_is_connected(&folder.external_path, volumes));
            let local_connected = folder
                .is_none_or(|folder| letter_is_connected(&folder.local_path, volumes));
            let cloud_active = cloud_connected && folder.is_some_and(|folder| folder.cloud_checked);
            let local = group.local;
            let stored_external = group.external;
            let mut status = local
                .as_ref()
                .and_then(|row| row.health_status.clone())
                .or_else(|| {
                    stored_external
                        .as_ref()
                        .and_then(|row| row.health_status.clone())
                })
                .unwrap_or_else(|| "HEALTHY".to_string());
            if external_unmounted && status == "MISSING" && local.is_some() && stored_external.is_none()
            {
                status = "HEALTHY".to_string();
            }
            let external = if external_unmounted {
                None
            } else {
                stored_external
            };
            let local_side = side_from_index(local.as_ref(), local_connected);
            let external_side = side_from_index(external.as_ref(), !external_unmounted);
            let cloud_view = cloud_active.then(|| cloud_view_from(group.cloud.as_ref()));
            if cloud_active {
                status = audit::classify(&local_side, &external_side, cloud_view.as_ref()).as_str().to_string();
            }
            let disk_hashes_differ = local_side.present
                && external_side.present
                && local_side.live_sha256 != external_side.live_sha256;
            let disk_present = local_side.present || external_side.present;
            let cloud_present = cloud_view.as_ref().is_some_and(|cloud| cloud.present);
            let hash_omitted = cloud_view.as_ref().is_some_and(|cloud| {
                cloud.present && cloud.quickxor.as_deref().unwrap_or("").is_empty() && disk_present
            });
            let note = match status.as_str() {
                "BIT_ROT" => {
                    Some("Hash changed while size and modified time stayed the same.".to_string())
                }
                "CORRUPT" if disk_hashes_differ => {
                    Some("Local and external hashes differ.".to_string())
                }
                "CORRUPT" => Some("OneDrive size or quickXorHash differs.".to_string()),
                "MISSING" if cloud_active && !cloud_present && disk_present => {
                    Some("Missing from OneDrive.".to_string())
                }
                "MISSING" => Some("Missing from a connected target.".to_string()),
                "HEALTHY" if hash_omitted => {
                    Some("OneDrive did not return quickXorHash.".to_string())
                }
                _ => None,
            };
            let id = local
                .as_ref()
                .or(external.as_ref())
                .map(|row| row.id.clone())
                .or_else(|| group.cloud.as_ref().map(|row| row.id.clone()))
                .unwrap_or_else(|| format!("{set_id}:{relative_path}"));
            DiscrepancyDto {
                id,
                set_id,
                relative_path,
                status,
                local: cell_from(local.as_ref(), false),
                external: if external_unmounted {
                    TargetCell {
                        present: false,
                        size_bytes: None,
                        mtime: None,
                        hash: None,
                        unmounted: true,
                        offline: false,
                        unscanned: false,
                    }
                } else {
                    cell_from(external.as_ref(), false)
                },
                cloud: if cloud_active {
                    cloud_cell(group.cloud.as_ref())
                } else {
                    TargetCell {
                        present: false,
                        size_bytes: None,
                        mtime: None,
                        hash: None,
                        unmounted: false,
                        offline: false,
                        unscanned: true,
                    }
                },
                note,
            }
        })
        .collect()
}

fn side_from_index(row: Option<&StoredIndex>, connected: bool) -> audit::Side {
    match row {
        Some(row) => audit::Side {
            connected,
            present: true,
            bit_rot: row.health_status.as_deref() == Some("BIT_ROT"),
            live_sha256: row.sha256_hash.clone().unwrap_or_default(),
            quickxor: row.quick_xor_hash.clone().unwrap_or_default(),
            size_bytes: row.file_size.max(0) as u64,
        },
        None => audit::Side {
            connected,
            present: false,
            bit_rot: false,
            live_sha256: String::new(),
            quickxor: String::new(),
            size_bytes: 0,
        },
    }
}

fn cloud_view_from(row: Option<&StoredCloud>) -> CloudView {
    match row {
        Some(row) => CloudView {
            connected: true,
            present: true,
            size_bytes: row.file_size.max(0) as u64,
            quickxor: row.quick_xor_hash.clone(),
        },
        None => CloudView {
            connected: true,
            present: false,
            size_bytes: 0,
            quickxor: None,
        },
    }
}

fn cloud_cell(row: Option<&StoredCloud>) -> TargetCell {
    match row {
        Some(row) => TargetCell {
            present: true,
            size_bytes: Some(row.file_size.max(0) as u64),
            mtime: Some(row.last_modified.clone()),
            hash: row.quick_xor_hash.clone(),
            unmounted: false,
            offline: false,
            unscanned: false,
        },
        None => TargetCell {
            present: false,
            size_bytes: None,
            mtime: None,
            hash: None,
            unmounted: false,
            offline: false,
            unscanned: false,
        },
    }
}

fn cell_from(row: Option<&StoredIndex>, unmounted: bool) -> TargetCell {
    match row {
        Some(row) => TargetCell {
            present: true,
            size_bytes: Some(row.file_size.max(0) as u64),
            mtime: Some(row.last_modified.clone()),
            hash: row.sha256_hash.clone(),
            unmounted,
            offline: false,
            unscanned: false,
        },
        None => TargetCell {
            present: false,
            size_bytes: None,
            mtime: None,
            hash: None,
            unmounted,
            offline: false,
            unscanned: false,
        },
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
            // App-data directory for guardsenpai.db only. This is not a watched backup path.
            std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
            let db = Database::open(&dir.join("guardsenpai.db")).map_err(|err| err.to_string())?;
            if let Ok(volumes) = list_connected_volumes() {
                db.apply_volumes(&volumes)?;
            }
            drives::spawn_listener(app.handle().clone(), db.clone());
            let client_config_paths = oauth::config_paths(&dir);
            oauth::install_config_paths(client_config_paths.clone());
            app.manage(AppState {
                db,
                client_config_paths,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_watched_folders,
            add_watched_folder,
            remove_watched_folder,
            list_drive_mounts,
            audit_folder_set,
            list_discrepancies,
            list_audit_logs,
            onedrive_status,
            onedrive_connect,
            onedrive_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("error while running Guard Senpai");
}
