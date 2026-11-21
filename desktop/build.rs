fn main() {
    // App commands are listed so a capability can grant them by name (the update buttons live in the web UI).
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&["update_check", "update_install"])),
    )
    .expect("failed to run the Tauri build step");
}
