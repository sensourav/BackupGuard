use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::db::Database;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub drive_letter: String,
    pub label: String,
}

pub fn drive_letter(path: &str) -> Option<String> {
    let mut chars = path.chars();
    let first = chars.next()?;
    let second = chars.next()?;
    if first.is_ascii_alphabetic() && second == ':' {
        Some(format!("{}:", first.to_ascii_uppercase()))
    } else {
        None
    }
}

pub fn list_connected_volumes() -> Result<Vec<Volume>, String> {
    query_logical_drives()
}

pub fn spawn_listener(app: AppHandle, db: Database) {
    std::thread::spawn(move || {
        let mut announced = false;
        loop {
            match list_connected_volumes() {
                Ok(volumes) => match db.apply_volumes(&volumes) {
                    Ok(changed) => {
                        if changed || !announced {
                            if let Ok(mounts) = db.list_drives() {
                                let _ = app.emit("drive-mounts-changed", &mounts);
                                announced = true;
                            }
                        }
                    }
                    Err(err) => eprintln!("drive mount update failed: {err}"),
                },
                Err(err) => eprintln!("drive poll failed: {err}"),
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

#[cfg(windows)]
fn query_logical_drives() -> Result<Vec<Volume>, String> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};

    const DRIVE_UNKNOWN: u32 = 0;
    const DRIVE_NO_ROOT_DIR: u32 = 1;

    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return Err("GetLogicalDrives returned no drives".to_string());
    }

    let mut volumes = Vec::new();
    for index in 0..26 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index) as char;
        let root = format!("{letter}:\\");
        let mut wide: Vec<u16> = root.encode_utf16().collect();
        wide.push(0);
        let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
        if drive_type == DRIVE_UNKNOWN || drive_type == DRIVE_NO_ROOT_DIR {
            continue;
        }
        volumes.push(Volume {
            drive_letter: format!("{letter}:"),
            label: volume_label(wide.as_ptr()),
        });
    }
    Ok(volumes)
}

#[cfg(windows)]
fn volume_label(root: *const u16) -> String {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    let mut name = [0u16; 261];
    let ok = unsafe {
        GetVolumeInformationW(
            root,
            name.as_mut_ptr(),
            name.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        return String::new();
    }
    let end = name.iter().position(|unit| *unit == 0).unwrap_or(name.len());
    String::from_utf16_lossy(&name[..end])
}

#[cfg(not(windows))]
fn query_logical_drives() -> Result<Vec<Volume>, String> {
    Err("drive detection is implemented for Windows".to_string())
}

pub fn letter_is_connected(path: &str, volumes: &[Volume]) -> bool {
    match drive_letter(path) {
        Some(letter) => volumes
            .iter()
            .any(|volume| volume.drive_letter.eq_ignore_ascii_case(&letter)),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_drive_letters() {
        assert_eq!(drive_letter("E:/Backups/DJI/").as_deref(), Some("E:"));
        assert_eq!(drive_letter(r"c:\Backups").as_deref(), Some("C:"));
        assert_eq!(drive_letter("/OneDrive/Backups/DJI/"), None);
    }

    #[test]
    fn windows_reports_the_system_drive() {
        let volumes = list_connected_volumes().expect("drive query");
        assert!(
            volumes.iter().any(|volume| volume.drive_letter == "C:"),
            "expected C: among {volumes:?}"
        );
    }

    #[test]
    fn missing_letter_is_not_connected() {
        let volumes = vec![Volume {
            drive_letter: "C:".to_string(),
            label: "OS".to_string(),
        }];
        assert!(letter_is_connected("C:/Backups/DJI/", &volumes));
        assert!(!letter_is_connected("E:/Backups/DJI/", &volumes));
    }
}
