// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The Vault first, before any other thread starts: on Linux every thread then shares it.
    plenipo_capabilities::OsSecretStore::prepare();
    // Tool relay mode (Phase 7): an AI tool started Plenipo as its MCP server for a worker;
    // pass messages to the running Plenipo. No window, tray, or webview.
    if let Some(code) = plenipo_capabilities::relay::maybe_run_from_args(std::env::args()) {
        std::process::exit(code);
    }
    // Ollama bridge mode (ADR-017): a task or sign-in check for Ollama's cloud models, sent to
    // the Ollama service on this PC.
    if let Some(code) =
        plenipo_runtime::agent::ollama::bridge::maybe_run_from_args(std::env::args())
    {
        std::process::exit(code);
    }
    // Paid helper mode (Phase 16 Wave 3, ADR-085, ADR-086): a paid task or key check, sent to
    // its service's fixed address, each checked by Guard's rules; the key comes on stdin.
    if let Some(code) = plenipo_capabilities::paid::helper::maybe_run_from_args(std::env::args()) {
        std::process::exit(code);
    }
    // Uninstall mode (Phase 13): the uninstaller asks Plenipo to forget the secrets it kept in
    // Windows Credential Manager, when the owner chose to delete their Plenipo data.
    if let Some(code) = plenipo_desktop_lib::uninstall::maybe_run_from_args(std::env::args()) {
        std::process::exit(code);
    }
    // Diagnostic child mode: harmless scenarios used by the runtime supervisor. Handled
    // before Tauri initializes so no window, tray, or webview is ever created.
    if let Some(code) = plenipo_runtime::diagnostic::maybe_run_from_args(std::env::args()) {
        std::process::exit(code);
    }
    std::process::exit(plenipo_desktop_lib::run());
}
