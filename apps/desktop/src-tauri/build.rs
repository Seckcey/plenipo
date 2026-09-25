use std::path::PathBuf;

fn main() {
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    // tauri-build embeds its Windows app manifest (Common-Controls v6) into the app
    // binary only. Test binaries that link Tauri then fail to start with
    // STATUS_ENTRYPOINT_NOT_FOUND. On Windows MSVC we skip tauri-build's manifest and
    // embed the same one via linker args, which apply to every target (app + tests).
    let windows_attributes = if windows_msvc {
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };

    // Declare every app command so each one needs an explicit capability grant
    // (see capabilities/default.json). Unlisted commands are unreachable from the UI.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(windows_attributes)
            .app_manifest(
                tauri_build::AppManifest::new().commands(&["get_app_info", "frontend_ready"]),
            ),
    )
    .expect("failed to run tauri-build");

    if windows_msvc {
        let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}
