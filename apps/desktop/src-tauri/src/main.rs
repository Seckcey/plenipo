// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Tool relay mode (Phase 7): an AI tool started Plenipo as its MCP server for a worker;
    // pass messages to the running Plenipo. No window, tray, or webview.
    if let Some(code) = plenipo_capabilities::relay::maybe_run_from_args(std::env::args()) {
        std::process::exit(code);
    }
    // Diagnostic child mode: harmless scenarios used by the runtime supervisor. Handled
    // before Tauri initializes so no window, tray, or webview is ever created.
    if let Some(code) = plenipo_runtime::diagnostic::maybe_run_from_args(std::env::args()) {
        std::process::exit(code);
    }
    std::process::exit(plenipo_desktop_lib::run());
}
