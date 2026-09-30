fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "list_watched_folders",
        "add_watched_folder",
        "remove_watched_folder",
        "list_drive_mounts",
        "audit_folder_set",
        "list_discrepancies",
        "list_audit_logs",
        "onedrive_status",
        "onedrive_connect",
        "onedrive_disconnect",
    ]);
    if let Err(error) = tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(manifest),
    ) {
        panic!("failed to run tauri-build: {error}");
    }
}
