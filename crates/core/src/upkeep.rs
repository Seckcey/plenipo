//! Keeping Plenipo dependable (Phase 13): starting and closing (ADR-036, background work),
//! recovery after a crash or a Windows restart, the diagnostics file, and updates (ADR-037).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What closing the window does (Settings → Start and close).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CloseWindow {
    /// With work going, the window hides and the work goes on; with nothing going, Plenipo
    /// quits (how 1.8.0 worked).
    #[default]
    KeepWhileWorking,
    /// Closing only hides the window; Quit Plenipo in the tray menu ends it.
    AlwaysKeep,
    /// Closing the window is the same as Quit Plenipo: work stops cleanly and is recorded.
    Quit,
}

/// Settings → Start and close.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartAndClose {
    /// Windows starts Plenipo in the tray when you sign in.
    pub start_with_windows: bool,
    /// This computer can do it (Windows; not in tests).
    pub can_start_with_windows: bool,
    pub close_window: CloseWindow,
}

/// A change to Settings → Start and close.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StartAndCloseInput {
    pub start_with_windows: bool,
    pub close_window: CloseWindow,
}

/// How the last run of Plenipo ended, when it did not end cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RecoveryCause {
    /// Plenipo closed unexpectedly (it crashed, or was ended from Task Manager).
    Crash,
    /// Windows restarted, or the computer lost power, while Plenipo was running.
    WindowsRestart,
    /// Plenipo stopped while it was changing the Ledger's layout (a migration).
    LayoutChange,
    /// The note Plenipo keeps while it runs was damaged, so the cause is unknown.
    Unknown,
}

/// A task that stopped because Plenipo did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StoppedTask {
    pub task_id: String,
    /// The first line of what was asked.
    pub objective: String,
    /// Who was doing it (a position's title, or the AI tool of a conversation).
    pub who: Option<String>,
    /// Run again is possible (it came from a position or a conversation).
    pub can_run_again: bool,
    /// Run again has been used for it (its new task).
    pub run_again_as: Option<String>,
}

/// What Plenipo found when it started after an unclean end, and what stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Recovery {
    /// The Ledger event that records it.
    pub id: String,
    pub cause: RecoveryCause,
    /// The last time the previous run was known to be alive.
    #[ts(type = "number | null")]
    pub last_seen_at: Option<u64>,
    #[ts(type = "number")]
    pub found_at: u64,
    pub previous_version: Option<String>,
    pub stopped_tasks: Vec<StoppedTask>,
    /// Programs (runs) that were stopped.
    pub stopped_programs: u32,
}

/// The window's page stopped and was brought back (ADR-036 item 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowRecovery {
    #[ts(type = "number")]
    pub at: u64,
    /// `true`: the window was closed and opened again; `false`: its page was reloaded.
    pub reopened: bool,
}

/// A setting document Plenipo could not read at start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsProblem {
    /// Which settings (`guard`, `routing`, …).
    pub key: String,
    /// What they are, in plain words ("Permissions").
    pub label: String,
    /// What is wrong, and what it means for now.
    pub message: String,
}

/// Everything recovery has to tell the owner at the moment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryStatus {
    pub recovery: Option<Recovery>,
    pub window: Option<WindowRecovery>,
    pub settings_problems: Vec<SettingsProblem>,
}

/// A diagnostics file Plenipo saved (Settings → Diagnostics).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiagnosticsFile {
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub created_at: u64,
    /// What is in it, one line each.
    pub contents: Vec<String>,
}

/// Where an update check got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateState {
    /// Not checked yet since Plenipo started.
    NotChecked,
    Checking,
    UpToDate,
    /// A newer version is ready to install.
    Available,
    Installing,
    /// The last check or install did not work (see `message`).
    Failed,
}

/// Settings → Updates (ADR-037).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateStatus {
    /// This version of Plenipo.
    pub version: String,
    pub state: UpdateState,
    /// This copy can install updates (it was built by the Release workflow, with the updater
    /// key's public half inside it).
    pub can_install: bool,
    #[ts(type = "number | null")]
    pub last_checked_at: Option<u64>,
    /// The newer version, when there is one.
    pub available: Option<AvailableUpdate>,
    /// What happened, in plain words (a failure, or why installing is not possible).
    pub message: Option<String>,
    /// Where to download versions by hand.
    pub releases_page: String,
}

/// A newer version of Plenipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AvailableUpdate {
    pub version: String,
    /// Its release notes, as published.
    pub notes: String,
    pub published: Option<String>,
}
