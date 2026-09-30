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
    "get_activity",
    "get_scope_events",
    "get_task_events",
    "get_project_record",
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
    "get_home",
    "get_task_record",
    "get_notice_settings",
    "set_notice_settings",
    "send_test_notice",
    "get_local_paths",
    "rename_organization",
    "set_organization_titles",
    "create_role",
    "create_department",
    "update_department",
    "create_project",
    "update_project",
    "archive_project",
    "set_model_rule",
    "set_role_learns",
    "set_agent_learning",
    "create_specialty",
    "update_specialty",
    "remove_specialty",
    "archive_department",
    "bring_back_position",
    "bring_back_project",
    "bring_back_department",
    "preview_delete_for_good",
    "delete_for_good",
    "save_to_workforce",
    "hire_from_workforce",
    "delete_saved_agent",
    // Phase 18: the organization canvas, lending, Watch, and the owner's tile.
    "place_tiles",
    "tidy_up",
    "retarget_oversight",
    "get_live_view",
    "lend_agent",
    "send_home",
    "get_watch",
    "get_watch_change",
    "subscribe_watch",
    "unsubscribe_watch",
    "get_owner_profile",
    "set_owner_profile",
    // Phase 19: the AI tools page (sign-in goes through `open_terminal`).
    "get_ai_tools",
    "check_ai_tool",
    "check_ai_tool_versions",
    "get_ai_tool_usage",
    "update_ai_tool",
    "cancel_ai_tool_update",
    "set_ai_tools_auto_update",
    "set_ai_tool_payment",
    // Phase 16 Wave 3 (ADR-085): paid AI keys.
    "save_paid_key",
    "remove_paid_key",
    // Phase 16 Wave 3 (ADR-085): spending caps.
    "get_spending",
    "set_spending_cap",
    "remove_spending_cap",
    // Phase 20: Settings → Connections (signing in happens in the owner's own browser).
    "get_connections",
    "connect_connection",
    "cancel_connection_sign_in",
    "disconnect_connection",
    "set_connection_parts",
    "set_connection_access",
    "set_connection_send_list",
    "set_connection_own_app",
    // Phase 20 part 20B: the owner's own Slack or Google app, and more than one Slack workspace.
    "save_connection_app",
    "add_connection",
    "remove_connection",
    // Phase 20 part 20C: keys typed into a card (HubSpot, Stripe, the website), and add-on tools.
    "save_connection_key",
    "add_add_on",
    "change_add_on",
    "remove_add_on",
    "check_add_on_tools",
    "set_add_on_tools",
    "hire_position",
    "fill_position",
    "vacate_position",
    "update_position",
    "move_position",
    "archive_position",
    "assign_oversight",
    "end_oversight",
    "give_objective",
    "get_routing",
    "save_model",
    "remove_model",
    "set_role_policy",
    "set_routing_options",
    "clear_usage_limit",
    "get_permissions",
    "save_permission_set",
    "remove_permission_set",
    "assign_permissions",
    "set_command_rules",
    "set_blocked_files",
    "set_sensitive_rule",
    "set_guard_options",
    "save_secret",
    "remove_secret",
    "get_approvals",
    "resolve_approval",
    "revoke_grant",
    "set_up_development",
    "get_objective_report",
    "get_project_work",
    "remove_workspace",
    "update_role",
    "get_control_status",
    "stop_all_control",
    "take_over_control",
    "allow_control",
    "set_website_rules",
    "set_switches",
    "get_learning",
    "set_learning",
    "set_role_learning",
    "decide_lesson",
    "remove_lesson",
    "get_browser_status",
    "set_browser_choice",
    "open_browser",
    "get_screenshot",
    "get_servers",
    "save_server",
    "remove_server",
    "check_server_identity",
    "test_server",
    "stop_server_command",
    "get_terminal_settings",
    "set_terminal_shell",
    "open_terminal",
    "write_terminal",
    "resize_terminal",
    "close_terminal",
    "get_recovery_status",
    "run_again",
    "dismiss_recovery",
    "dismiss_window_recovery",
    "window_alive",
    "reset_settings",
    "get_start_and_close",
    "set_start_and_close",
    "list_ledger_backups",
    "restore_ledger_backup",
    "cancel_ledger_restore",
    "save_diagnostics_file",
    "get_update_status",
    "check_for_updates",
    "install_update",
    "prepare_pop_out",
    "focus_pop_out",
    "reset_pop_outs",
    "get_file_roots",
    "list_folder",
    "read_file",
    "save_file",
    "open_file_outside",
    "show_in_folder",
    "get_changing_files",
    "close_pop_out",
    "get_organizations",
    "create_organization",
    "switch_organization",
    "open_organization_window",
    "archive_organization",
    "bring_back_organization",
    "preview_delete_organization",
    "delete_organization_for_good",
    // Phase 11A: Settings → License.
    "get_license",
    "enter_license_key",
    "remove_license_key",
    "check_license_now",
];

fn main() {
    // Where updates come from, and the updater key's public half, are built in by the Release
    // workflow (ADR-038); a change to either rebuilds the app.
    println!("cargo:rerun-if-env-changed=PLENIPO_UPDATE_ENDPOINT");
    println!("cargo:rerun-if-env-changed=PLENIPO_UPDATER_PUBLIC_KEY");
    // The app ID Plenipo signs in to Microsoft 365 with (public, not a secret), and the
    // stand-in for the connections' services in copies built for the end-to-end tests
    // (Phase 20, ADR-065 §6).
    println!("cargo:rerun-if-env-changed=PLENIPO_MICROSOFT_APP_ID");
    // 8 West's Slack app's client ID (public, not a secret; ADR-070 §3).
    println!("cargo:rerun-if-env-changed=PLENIPO_SLACK_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=PLENIPO_CONNECTIONS_STAND_IN");
    // The stand-in for 8 West's license check in copies built for the tests (Phase 11A).
    println!("cargo:rerun-if-env-changed=PLENIPO_LICENSE_STAND_IN");
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
