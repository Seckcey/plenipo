use std::path::PathBuf;

/// Every app command. Each must also be granted in capabilities/default.json.
const COMMANDS: &[&str] = &[
    "get_app_info",
    "frontend_ready",
    "get_runtime_overview",
    "start_execution",
    "cancel_execution",
    "get_execution_output",
    "get_ledger_status",
    "list_tasks",
    "get_task_timeline",
    "list_recent_events",
    "create_synthetic_task",
    "advance_synthetic_task",
    "run_integrity_check",
    "create_ledger_backup",
    "export_ledger",
    "get_agent_overview",
    "refresh_agent_runtimes",
    "get_agent_session",
    "start_agent_session",
    "resume_agent_session",
    "cancel_agent_turn",
    "close_agent_session",
    "get_task_handoffs",
    "get_task_tree",
    "get_liaison_overview",
    "get_organization",
    "get_work",
    "rename_organization",
    "set_organization_titles",
    "create_role",
    "create_department",
    "update_department",
    "remove_department",
    "create_project",
    "update_project",
    "archive_project",
    "hire_position",
    "fill_position",
    "vacate_position",
    "update_position",
    "move_position",
    "archive_position",
    "assign_oversight",
    "end_oversight",
    "give_objective",
];

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
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
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
