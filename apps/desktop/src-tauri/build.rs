fn main() {
    // Declare every app command so each one needs an explicit capability grant
    // (see capabilities/default.json). Unlisted commands are unreachable from the UI.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["get_app_info", "frontend_ready"]),
    ))
    .expect("failed to run tauri-build");
}
