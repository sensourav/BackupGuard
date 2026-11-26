use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkedFile {
    pub name: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkOutcome {
    pub files: Vec<WalkedFile>,
    pub warnings: Vec<String>,
}

/// The only file-handle opener in this crate. Read is enabled and write is disabled.
/// Create, append, and truncate are left unset so opening a path cannot create it.
#[allow(dead_code)]
pub fn open_read_only(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).write(false).open(path)
}

pub fn walk_directory(root: &Path) -> Result<WalkOutcome, String> {
    let meta = fs::symlink_metadata(root)
        .map_err(|err| format!("{}: {err}", root.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "{} is a link; refusing to follow it",
            root.display()
        ));
    }
    if !meta.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }

    let mut files = Vec::new();
    let mut warnings = Vec::new();
    visit(root, root, &mut files, &mut warnings);
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(WalkOutcome { files, warnings })
}

fn visit(root: &Path, dir: &Path, files: &mut Vec<WalkedFile>, warnings: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            warnings.push(format!("{}: {err}", dir.display()));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warnings.push(err.to_string());
                continue;
            }
        };
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(err) => {
                warnings.push(format!("{}: {err}", path.display()));
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            visit(root, &path, files, warnings);
            continue;
        }
        if !meta.is_file() {
            continue;
        }

        let relative = match path.strip_prefix(root) {
            Ok(relative) => relative,
            Err(_) => {
                warnings.push(format!(
                    "{} is outside the scan root",
                    path.display()
                ));
                continue;
            }
        };
        files.push(WalkedFile {
            name: file_name(&path),
            relative_path: to_relative(relative),
            size_bytes: meta.len(),
            modified: meta
                .modified()
                .ok()
                .map(modified_iso)
                .unwrap_or_default(),
        });
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn to_relative(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn modified_iso(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
pub fn fixture_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "guardsenpai-{name}-{}",
        uuid::Uuid::new_v4()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_nested_files_without_creating_them() {
        let root = fixture_root("walk");
        let nested = root.join("2026-09-16");
        fs::create_dir_all(&nested).unwrap();
        let file_path = nested.join("clip.txt");
        fs::write(&file_path, b"abc").unwrap();

        let outcome = walk_directory(&root).unwrap();
        assert_eq!(outcome.files.len(), 1);
        assert_eq!(outcome.files[0].name, "clip.txt");
        assert_eq!(outcome.files[0].relative_path, "2026-09-16/clip.txt");
        assert_eq!(outcome.files[0].size_bytes, 3);
        assert!(!outcome.files[0].modified.is_empty());
        assert_eq!(fs::read(&file_path).unwrap(), b"abc");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_only_open_does_not_create_a_missing_file() {
        let root = fixture_root("open");
        fs::create_dir_all(&root).unwrap();
        let missing = root.join("missing.txt");
        assert!(open_read_only(&missing).is_err());
        assert!(!missing.exists());
        let _ = fs::remove_dir_all(root);
    }
}
