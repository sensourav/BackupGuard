use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use crate::db::{now_iso, Database, IndexRow};
use crate::graph::{self, CloudFile, CloudScope};
use crate::hash::{hash_file, CHUNK_BYTES};
use crate::walker::{walk_directory, WalkedFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditScope {
    Full,
    Folder(String),
    File(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Missing,
    Corrupt,
    BitRot,
}

impl HealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "HEALTHY",
            Self::Missing => "MISSING",
            Self::Corrupt => "CORRUPT",
            Self::BitRot => "BIT_ROT",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BaselineDecision {
    pub sha256: String,
    pub quickxor: String,
    pub bit_rot: bool,
}

#[derive(Debug, Clone)]
pub struct Side {
    pub connected: bool,
    pub present: bool,
    pub bit_rot: bool,
    pub live_sha256: String,
    pub quickxor: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CloudView {
    pub connected: bool,
    pub present: bool,
    pub size_bytes: u64,
    pub quickxor: Option<String>,
}

pub enum CloudAudit {
    Skipped,
    Failed(String),
    Token(String),
}

pub fn parse_scope(scope: &str, relative_path: Option<&str>) -> Result<AuditScope, String> {
    match scope {
        "full" => Ok(AuditScope::Full),
        "folder" => {
            let folder = normalize_relative(relative_path.unwrap_or(""));
            if !is_daily_folder(&folder) {
                return Err("folder audit expects a YYYY-MM-DD path".to_string());
            }
            Ok(AuditScope::Folder(folder))
        }
        "file" => {
            let file = normalize_relative(relative_path.unwrap_or(""));
            if file.is_empty() || file.ends_with('/') {
                return Err("file audit expects a relative file path".to_string());
            }
            reject_escape(&file)?;
            Ok(AuditScope::File(file))
        }
        _ => Err("audit scope must be full, folder, or file".to_string()),
    }
}

pub fn decide_baseline(
    size_bytes: u64,
    modified: &str,
    live_sha256: &str,
    live_quickxor: &str,
    previous: Option<&IndexRow>,
) -> BaselineDecision {
    if let Some(previous) = previous {
        let same_metadata =
            previous.file_size == size_bytes as i64 && previous.last_modified == modified;
        if same_metadata {
            if let Some(baseline) = previous.sha256_hash.as_deref() {
                if baseline != live_sha256 {
                    return BaselineDecision {
                        sha256: baseline.to_string(),
                        quickxor: previous
                            .quick_xor_hash
                            .clone()
                            .unwrap_or_else(|| live_quickxor.to_string()),
                        bit_rot: true,
                    };
                }
            }
        }
    }
    BaselineDecision {
        sha256: live_sha256.to_string(),
        quickxor: live_quickxor.to_string(),
        bit_rot: false,
    }
}

pub fn classify(local: &Side, external: &Side, cloud: Option<&CloudView>) -> HealthStatus {
    if (local.present && local.bit_rot) || (external.present && external.bit_rot) {
        return HealthStatus::BitRot;
    }
    let disk = classify_disks(local, external);
    let Some(cloud) = cloud.filter(|cloud| cloud.connected) else {
        return disk;
    };
    if disk == HealthStatus::Corrupt {
        return HealthStatus::Corrupt;
    }
    let reference = disk_reference(local).or_else(|| disk_reference(external));
    if cloud.present {
        if let Some(reference) = reference {
            if reference.size_bytes != cloud.size_bytes {
                return HealthStatus::Corrupt;
            }
            if let Some(remote) = cloud.quickxor.as_deref().filter(|value| !value.is_empty()) {
                if remote != reference.quickxor {
                    return HealthStatus::Corrupt;
                }
            }
            return disk;
        }
        if local.connected || external.connected {
            return HealthStatus::Missing;
        }
        return HealthStatus::Healthy;
    }
    if reference.is_some() {
        return HealthStatus::Missing;
    }
    disk
}

fn classify_disks(local: &Side, external: &Side) -> HealthStatus {
    if local.connected && external.connected && local.present && external.present {
        if local.live_sha256 != external.live_sha256 {
            return HealthStatus::Corrupt;
        }
        return HealthStatus::Healthy;
    }
    if local.connected && external.connected && local.present != external.present {
        return HealthStatus::Missing;
    }
    HealthStatus::Healthy
}

fn disk_reference(side: &Side) -> Option<&Side> {
    if side.connected && side.present {
        Some(side)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditProgress {
    pub relative_path: String,
    pub file_percent: u8,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub folder_id: String,
    pub last_scanned: String,
    pub duration_ms: u64,
    pub bytes_processed: u64,
    pub total_files: u64,
    pub healthy_count: u64,
    pub missing_count: u64,
    pub corrupt_count: u64,
    pub bitrot_count: u64,
    pub warnings: Vec<String>,
}

pub struct AuditRequest {
    pub folder_id: String,
    pub set_name: String,
    pub local_path: String,
    pub external_path: String,
    pub cloud_path: String,
    pub scope: AuditScope,
    pub local_connected: bool,
    pub external_connected: bool,
    pub cloud: CloudAudit,
}

struct HashedTarget {
    size_bytes: u64,
    modified: String,
    live_sha256: String,
    live_quickxor: String,
    baseline_sha256: String,
    baseline_quickxor: String,
    bit_rot: bool,
}

pub fn run_audit(
    db: &Database,
    request: &AuditRequest,
    mut progress: impl FnMut(&AuditProgress),
) -> Result<AuditReport, String> {
    let started = Instant::now();
    let mut warnings = Vec::new();
    let mut buffer = vec![0u8; CHUNK_BYTES];

    let local_files = if request.local_connected {
        match collect_files(Path::new(&request.local_path), &request.scope) {
            Ok(files) => Some(files),
            Err(err) => {
                warnings.push(format!("local: {err}"));
                None
            }
        }
    } else {
        warnings.push("local target is unmounted".to_string());
        None
    };
    let external_files = if request.external_connected {
        match collect_files(Path::new(&request.external_path), &request.scope) {
            Ok(files) => Some(files),
            Err(err) => {
                warnings.push(format!("external: {err}"));
                None
            }
        }
    } else {
        None
    };

    let total_bytes = local_files.iter().flatten().map(|file| file.size_bytes).sum::<u64>()
        + external_files
            .iter()
            .flatten()
            .map(|file| file.size_bytes)
            .sum::<u64>();

    let local_previous = db.index_map(&request.folder_id, "local")?;
    let external_previous = db.index_map(&request.folder_id, "external")?;

    let mut bytes_processed = 0u64;
    let local_hashed = hash_target(
        Path::new(&request.local_path),
        local_files.as_deref(),
        &local_previous,
        &mut buffer,
        total_bytes,
        &mut bytes_processed,
        &mut progress,
    )?;
    let external_hashed = hash_target(
        Path::new(&request.external_path),
        external_files.as_deref(),
        &external_previous,
        &mut buffer,
        total_bytes,
        &mut bytes_processed,
        &mut progress,
    )?;

    let cloud_files = match &request.cloud {
        CloudAudit::Skipped => None,
        CloudAudit::Failed(message) => {
            warnings.push(message.clone());
            None
        }
        CloudAudit::Token(token) => {
            match graph::list_files(
                &request.cloud_path,
                &cloud_scope(&request.scope),
                token,
                |path| {
                    progress(&AuditProgress {
                        relative_path: path.to_string(),
                        file_percent: 0,
                        bytes_processed,
                        total_bytes,
                        phase: "cloud".to_string(),
                    });
                },
            ) {
                Ok(files) => Some(files),
                Err(err) => {
                    warnings.push(format!("OneDrive: {err}"));
                    None
                }
            }
        }
    };

    if matches!(request.scope, AuditScope::File(_))
        && local_files.as_ref().is_none_or(|files| files.is_empty())
        && external_files.as_ref().is_none_or(|files| files.is_empty())
        && cloud_files.as_ref().is_none_or(|files| files.is_empty())
    {
        db.set_cloud_checked(&request.folder_id, false)?;
        if warnings.is_empty() {
            return Err("file was not found on a connected target".to_string());
        }
        return Err(warnings.join("; "));
    }

    let mut paths: HashSet<String> = HashSet::new();
    if let Some(files) = &local_hashed {
        paths.extend(files.keys().cloned());
    }
    if let Some(files) = &external_hashed {
        paths.extend(files.keys().cloned());
    }
    if local_files.is_some() {
        for row in local_previous.values() {
            if in_scope(&row.relative_path, &request.scope) {
                paths.insert(row.relative_path.clone());
            }
        }
    }
    if external_files.is_some() {
        for row in external_previous.values() {
            if in_scope(&row.relative_path, &request.scope) {
                paths.insert(row.relative_path.clone());
            }
        }
    }
    if let Some(files) = &cloud_files {
        for file in files {
            if in_scope(&file.relative_path, &request.scope) {
                paths.insert(file.relative_path.clone());
            }
        }
    }

    let cloud_by_path: HashMap<String, &CloudFile> = cloud_files
        .as_ref()
        .map(|files| {
            files
                .iter()
                .map(|file| (file.relative_path.clone(), file))
                .collect()
        })
        .unwrap_or_default();

    let mut healthy = 0u64;
    let mut missing = 0u64;
    let mut corrupt = 0u64;
    let mut bitrot = 0u64;
    let mut ordered: Vec<String> = paths.into_iter().collect();
    ordered.sort();

    for relative_path in &ordered {
        let local_side = side_for(
            request.local_connected,
            local_files.is_some(),
            local_hashed.as_ref().and_then(|files| files.get(relative_path)),
        );
        let external_side = side_for(
            request.external_connected,
            external_files.is_some(),
            external_hashed
                .as_ref()
                .and_then(|files| files.get(relative_path)),
        );
        let cloud_view = cloud_files.as_ref().map(|_| cloud_view_for(cloud_by_path.get(relative_path).copied()));
        let status = classify(&local_side, &external_side, cloud_view.as_ref());
        let disk_status = classify(&local_side, &external_side, None);
        match status {
            HealthStatus::Healthy => healthy += 1,
            HealthStatus::Missing => missing += 1,
            HealthStatus::Corrupt => corrupt += 1,
            HealthStatus::BitRot => bitrot += 1,
        }
        persist_side(
            db,
            &request.folder_id,
            "local",
            relative_path,
            local_files.is_some(),
            local_hashed.as_ref().and_then(|files| files.get(relative_path)),
            disk_status,
        )?;
        persist_side(
            db,
            &request.folder_id,
            "external",
            relative_path,
            external_files.is_some(),
            external_hashed
                .as_ref()
                .and_then(|files| files.get(relative_path)),
            disk_status,
        )?;
    }

    if let Some(files) = &cloud_files {
        let existing = db.list_cloud_for_folder(&request.folder_id)?;
        for row in existing {
            if in_scope(&row.relative_path, &request.scope) {
                db.delete_cloud_path(&request.folder_id, &row.relative_path)?;
            }
        }
        for file in files {
            db.upsert_cloud(
                &request.folder_id,
                &file.relative_path,
                file.size_bytes,
                &file.modified,
                file.quickxor.as_deref(),
                file.sha256.as_deref(),
            )?;
        }
        db.set_cloud_checked(&request.folder_id, true)?;
    } else {
        db.set_cloud_checked(&request.folder_id, false)?;
    }

    let scanned_at = now_iso();
    let duration_ms = started.elapsed().as_millis() as u64;
    let summary = match &request.scope {
        AuditScope::Full => format!("Full audit of {}", request.set_name),
        AuditScope::Folder(folder) => format!("Folder audit {folder}"),
        AuditScope::File(file) => format!("File check {file}"),
    };
    db.insert_audit_log(
        &request.folder_id,
        &scanned_at,
        ordered.len() as i64,
        healthy as i64,
        missing as i64,
        corrupt as i64,
        bitrot as i64,
        duration_ms as i64,
        bytes_processed as i64,
        &summary,
    )?;
    db.mark_scanned(&request.folder_id, &scanned_at)?;

    Ok(AuditReport {
        folder_id: request.folder_id.clone(),
        last_scanned: scanned_at,
        duration_ms,
        bytes_processed,
        total_files: ordered.len() as u64,
        healthy_count: healthy,
        missing_count: missing,
        corrupt_count: corrupt,
        bitrot_count: bitrot,
        warnings,
    })
}

fn cloud_scope(scope: &AuditScope) -> CloudScope {
    match scope {
        AuditScope::Full => CloudScope::Full,
        AuditScope::Folder(folder) => CloudScope::Folder(folder.clone()),
        AuditScope::File(file) => CloudScope::File(file.clone()),
    }
}

fn cloud_view_for(file: Option<&CloudFile>) -> CloudView {
    match file {
        Some(file) => CloudView {
            connected: true,
            present: true,
            size_bytes: file.size_bytes,
            quickxor: file.quickxor.clone(),
        },
        None => CloudView {
            connected: true,
            present: false,
            size_bytes: 0,
            quickxor: None,
        },
    }
}

fn side_for(connected: bool, scanned: bool, hashed: Option<&HashedTarget>) -> Side {
    if let Some(file) = hashed {
        return         Side {
            connected,
            present: true,
            bit_rot: file.bit_rot,
            live_sha256: file.live_sha256.clone(),
            quickxor: file.live_quickxor.clone(),
            size_bytes: file.size_bytes,
        };
    }
    Side {
        connected: connected && scanned,
        present: false,
        bit_rot: false,
        live_sha256: String::new(),
        quickxor: String::new(),
        size_bytes: 0,
    }
}

fn persist_side(
    db: &Database,
    folder_id: &str,
    target: &str,
    relative_path: &str,
    scanned: bool,
    hashed: Option<&HashedTarget>,
    status: HealthStatus,
) -> Result<(), String> {
    if !scanned {
        return Ok(());
    }
    if let Some(file) = hashed {
        db.upsert_hashed_row(
            folder_id,
            target,
            relative_path,
            file.size_bytes,
            &file.modified,
            &file.baseline_sha256,
            &file.baseline_quickxor,
            status.as_str(),
        )?;
    } else {
        db.delete_index_path(folder_id, target, relative_path)?;
    }
    Ok(())
}

fn hash_target(
    root: &Path,
    files: Option<&[WalkedFile]>,
    previous: &HashMap<String, IndexRow>,
    buffer: &mut [u8],
    total_bytes: u64,
    bytes_processed: &mut u64,
    progress: &mut impl FnMut(&AuditProgress),
) -> Result<Option<HashMap<String, HashedTarget>>, String> {
    let Some(files) = files else {
        return Ok(None);
    };
    let mut hashed = HashMap::new();
    for file in files {
        let path = root.join(file.relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let size = file.size_bytes;
        let digest = hash_file(&path, buffer, |read| {
            let file_percent = if size == 0 {
                100
            } else {
                ((read.saturating_mul(100)) / size).min(100) as u8
            };
            progress(&AuditProgress {
                relative_path: file.relative_path.clone(),
                file_percent,
                bytes_processed: bytes_processed.saturating_add(read),
                total_bytes,
                phase: "hash".to_string(),
            });
        })?;
        *bytes_processed = bytes_processed.saturating_add(digest.bytes_read);
        let decision = decide_baseline(
            file.size_bytes,
            &file.modified,
            &digest.sha256_hex,
            &digest.quickxor_b64,
            previous.get(&file.relative_path),
        );
        hashed.insert(
            file.relative_path.clone(),
            HashedTarget {
                size_bytes: file.size_bytes,
                modified: file.modified.clone(),
                live_sha256: digest.sha256_hex,
                live_quickxor: digest.quickxor_b64,
                baseline_sha256: decision.sha256,
                baseline_quickxor: decision.quickxor,
                bit_rot: decision.bit_rot,
            },
        );
    }
    Ok(Some(hashed))
}

fn collect_files(root: &Path, scope: &AuditScope) -> Result<Vec<WalkedFile>, String> {
    match scope {
        AuditScope::File(relative) => Ok(single_file(root, relative)?.into_iter().collect()),
        AuditScope::Folder(_) | AuditScope::Full => {
            let outcome = walk_directory(root)?;
            Ok(outcome
                .files
                .into_iter()
                .filter(|file| in_scope(&file.relative_path, scope))
                .collect())
        }
    }
}

fn single_file(root: &Path, relative: &str) -> Result<Option<WalkedFile>, String> {
    let path = safe_join(root, relative)?;
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => Ok(None),
        Ok(meta) if meta.is_file() => Ok(Some(WalkedFile {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            relative_path: relative.to_string(),
            size_bytes: meta.len(),
            modified: crate::walker::modified_iso(
                meta.modified()
                    .map_err(|err| format!("{}: {err}", path.display()))?,
            ),
        })),
        Ok(_) => Err(format!("{} is not a file", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

fn in_scope(relative: &str, scope: &AuditScope) -> bool {
    match scope {
        AuditScope::Full => true,
        AuditScope::Folder(folder) => relative == folder || relative.starts_with(&format!("{folder}/")),
        AuditScope::File(file) => relative == file,
    }
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    reject_escape(relative)?;
    Ok(root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

fn reject_escape(relative: &str) -> Result<(), String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err("path must stay inside the watched folder".to_string());
    }
    Ok(())
}

fn normalize_relative(value: &str) -> String {
    value
        .trim()
        .trim_matches(['/', '\\'])
        .replace('\\', "/")
}

fn is_daily_folder(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, NewFolder};
    use std::fs;

    fn previous(size: i64, modified: &str, sha: Option<&str>) -> IndexRow {
        IndexRow {
            relative_path: "2026-09-16/clip.txt".to_string(),
            file_size: size,
            last_modified: modified.to_string(),
            sha256_hash: sha.map(str::to_string),
            quick_xor_hash: sha.map(|_| "baseline-xor".to_string()),
            health_status: Some("HEALTHY".to_string()),
        }
    }

    fn side(connected: bool, present: bool, rot: bool, sha: &str) -> Side {
        Side {
            connected,
            present,
            bit_rot: rot,
            live_sha256: sha.to_string(),
            quickxor: "xor".to_string(),
            size_bytes: 4,
        }
    }

    fn cloud(present: bool, size: u64, quickxor: Option<&str>) -> CloudView {
        CloudView {
            connected: true,
            present,
            size_bytes: size,
            quickxor: quickxor.map(str::to_string),
        }
    }

    #[test]
    fn unchanged_metadata_and_new_hash_is_bit_rot() {
        let prior = previous(4, "2026-09-16T00:00:00Z", Some("aaa"));
        let decision = decide_baseline(4, "2026-09-16T00:00:00Z", "bbb", "live-xor", Some(&prior));
        assert!(decision.bit_rot);
        assert_eq!(decision.sha256, "aaa");
        let status = classify(
            &side(true, true, true, "bbb"),
            &side(false, false, false, ""),
            None,
        );
        assert_eq!(status, HealthStatus::BitRot);
    }

    #[test]
    fn matching_hash_is_healthy() {
        let prior = previous(4, "2026-09-16T00:00:00Z", Some("aaa"));
        let decision = decide_baseline(4, "2026-09-16T00:00:00Z", "aaa", "live-xor", Some(&prior));
        assert!(!decision.bit_rot);
        let status = classify(
            &side(true, true, false, "aaa"),
            &side(true, true, false, "aaa"),
            None,
        );
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn different_live_hashes_are_corrupt() {
        let status = classify(
            &side(true, true, false, "local"),
            &side(true, true, false, "external"),
            None,
        );
        assert_eq!(status, HealthStatus::Corrupt);
    }

    #[test]
    fn unmounted_external_is_not_missing() {
        let status = classify(
            &side(true, true, false, "local"),
            &side(false, false, false, ""),
            None,
        );
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn audit_leaves_file_bytes_unchanged_and_records_counts() {
        let local = crate::walker::fixture_root("audit-local");
        fs::create_dir_all(local.join("2026-09-16")).unwrap();
        let file_path = local.join("2026-09-16").join("clip.txt");
        fs::write(&file_path, b"same").unwrap();
        let db = Database::open_in_memory().unwrap();
        let folder = db
            .add_folder(NewFolder {
                name: "Photos".to_string(),
                local_path: local.display().to_string(),
                external_path: "Z:/Backups/Photos/".to_string(),
                cloud_path: "/OneDrive/Backups/Photos/".to_string(),
            })
            .unwrap();
        let report = run_audit(
            &db,
            &AuditRequest {
                folder_id: folder.id.clone(),
                set_name: folder.name,
                local_path: folder.local_path,
                external_path: folder.external_path,
                cloud_path: folder.cloud_path,
                scope: AuditScope::Full,
                local_connected: true,
                external_connected: false,
                cloud: CloudAudit::Skipped,
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(report.healthy_count, 1);
        assert_eq!(report.missing_count, 0);
        assert_eq!(report.bitrot_count, 0);
        assert_eq!(fs::read(&file_path).unwrap(), b"same");
        let _ = fs::remove_dir_all(local);
    }

    #[test]
    fn cloud_match_is_healthy_and_mismatch_is_corrupt() {
        let local = side(true, true, false, "aaa");
        let external = side(false, false, false, "");
        assert_eq!(
            classify(&local, &external, Some(&cloud(true, 4, Some("xor")))),
            HealthStatus::Healthy
        );
        assert_eq!(
            classify(&local, &external, Some(&cloud(false, 0, None))),
            HealthStatus::Missing
        );
        assert_eq!(
            classify(&local, &external, Some(&cloud(true, 9, Some("xor")))),
            HealthStatus::Corrupt
        );
        assert_eq!(
            classify(&local, &external, Some(&cloud(true, 4, Some("other")))),
            HealthStatus::Corrupt
        );
        assert_eq!(
            classify(&local, &external, Some(&cloud(true, 4, None))),
            HealthStatus::Healthy
        );
        assert_eq!(
            classify(&local, &external, None),
            HealthStatus::Healthy
        );
    }

    #[test]
    fn bit_rot_wins_over_a_cloud_mismatch() {
        let status = classify(
            &side(true, true, true, "bbb"),
            &side(false, false, false, ""),
            Some(&cloud(true, 9, Some("other"))),
        );
        assert_eq!(status, HealthStatus::BitRot);
    }
}
