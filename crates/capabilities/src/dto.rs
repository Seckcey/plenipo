//! Capability broker DTOs shared with the frontend (camelCase on the wire).

use plenipo_guard::{Capability, GuardSettings, Level, Risk, SensitiveKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where an approval stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl From<plenipo_ledger::ApprovalState> for ApprovalStatus {
    fn from(s: plenipo_ledger::ApprovalState) -> Self {
        match s {
            plenipo_ledger::ApprovalState::Pending => Self::Pending,
            plenipo_ledger::ApprovalState::Approved => Self::Approved,
            plenipo_ledger::ApprovalState::Rejected => Self::Rejected,
            plenipo_ledger::ApprovalState::Expired => Self::Expired,
        }
    }
}

/// An approval card: what a worker wants to do, why it needs the owner, and the outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalView {
    pub id: String,
    pub task_id: String,
    pub status: ApprovalStatus,
    #[ts(type = "number")]
    pub requested_at: u64,
    #[ts(type = "number | null")]
    pub expires_at: Option<u64>,
    #[ts(type = "number | null")]
    pub resolved_at: Option<u64>,
    #[ts(optional)]
    pub resolved_by: Option<String>,
    /// The worker's title ("Backend Developer").
    pub worker: String,
    pub role: String,
    #[ts(optional)]
    pub project: Option<String>,
    #[ts(optional)]
    pub folder: Option<String>,
    #[ts(optional)]
    pub capability: Option<Capability>,
    /// "Run programs".
    pub capability_label: String,
    /// What it wants to do, in a line ("Run npm publish").
    pub summary: String,
    /// The exact action: a command line, a path, a script.
    pub detail: String,
    /// Why it needs the owner.
    pub reason: String,
    #[ts(optional)]
    pub risk: Option<Risk>,
    pub risk_label: String,
    #[ts(optional)]
    pub sensitive: Option<SensitiveKind>,
    #[ts(optional)]
    pub sensitive_label: Option<String>,
    #[ts(optional)]
    pub session_id: Option<String>,
    #[ts(optional)]
    pub grant_id: Option<String>,
    /// A worker is waiting for the answer right now.
    pub waiting: bool,
    /// The note recorded with the outcome.
    #[ts(optional)]
    pub note: Option<String>,
    /// The web page it concerns (Phase 10).
    #[ts(optional)]
    pub url: Option<String>,
    /// A screenshot of the page as it was when it asked (an artifact ID; `get_screenshot`).
    #[ts(optional)]
    pub screenshot: Option<String>,
    /// The server it concerns (Phase 11): its name, environment, and address.
    #[ts(optional)]
    pub server: Option<String>,
    #[ts(optional)]
    pub environment: Option<plenipo_guard::Environment>,
    #[ts(optional)]
    pub address: Option<String>,
}

/// Pending approvals (oldest first) and recent outcomes (newest first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalQueue {
    pub pending: Vec<ApprovalView>,
    pub recent: Vec<ApprovalView>,
}

/// One permission a grant holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GrantPermission {
    pub capability: Capability,
    pub label: String,
    pub level: Level,
}

/// A worker's permissions in use now: one step of one task (the plan's runtime grant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GrantView {
    pub grant_id: String,
    pub task_id: String,
    pub session_id: String,
    pub step: u32,
    #[ts(optional)]
    pub position_id: Option<String>,
    pub worker: String,
    pub role: String,
    #[ts(optional)]
    pub project: Option<String>,
    #[ts(optional)]
    pub folder: Option<String>,
    pub permissions: Vec<GrantPermission>,
    #[ts(type = "number")]
    pub opened_at: u64,
    pub revoked: bool,
    /// Tool calls carried out, blocked, and sent to the owner.
    pub used: u32,
    pub blocked: u32,
    pub asked: u32,
}

/// A request Guard blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlockedView {
    #[ts(type = "number")]
    pub at: u64,
    #[ts(optional)]
    pub task_id: Option<String>,
    pub worker: String,
    pub capability_label: String,
    pub summary: String,
    pub reason: String,
}

/// The operating system's protected storage for secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VaultStatus {
    pub available: bool,
    /// "Windows Credential Manager".
    pub label: String,
    #[ts(optional)]
    pub detail: Option<String>,
    /// IDs of secrets whose value is stored.
    pub stored: Vec<String>,
}

/// Plenipo's tool server for workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolsStatus {
    pub running: bool,
    pub detail: String,
}

/// Everything Settings → Permissions and the Approvals page show besides the queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PermissionsSnapshot {
    pub settings: GuardSettings,
    pub vault: VaultStatus,
    pub tools: ToolsStatus,
    /// Workers using permissions now.
    pub grants: Vec<GrantView>,
    /// Recently blocked requests, newest first.
    pub blocked: Vec<BlockedView>,
    pub notices: Vec<String>,
}

/// A kept screenshot, ready to show (Phase 10): its type and a `data:` URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Screenshot {
    pub mime: String,
    pub data_url: String,
}

// ---- Servers (Phase 11, ADR-025) ------------------------------------------------------------

/// Which sign-in values Plenipo keeps for a server (never the values).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StoredSignIn {
    pub key: bool,
    pub passphrase: bool,
    pub password: bool,
}

/// A server showed an identity other than the pinned one (and workers were stopped).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IdentityChange {
    pub algorithm: String,
    pub fingerprint: String,
    #[ts(type = "number")]
    pub at: u64,
}

/// A server as Settings → Servers shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerView {
    pub server: plenipo_guard::Server,
    /// `deploy@web01.example.com:22`.
    pub address: String,
    pub stored: StoredSignIn,
    /// The server showed a different identity after it was pinned.
    #[ts(optional)]
    pub identity_changed: Option<IdentityChange>,
    /// Workers connected to it now.
    pub connected: Vec<String>,
    /// What keeps workers from using it, if anything.
    #[ts(optional)]
    pub problem: Option<String>,
}

/// A role, and whether its permission set lets it connect to servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerRole {
    pub id: String,
    pub name: String,
    /// Its permission set allows "Connect to servers".
    pub can_connect: bool,
}

/// A kind of command, as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandClassInfo {
    pub class: plenipo_guard::CommandClass,
    pub label: String,
    pub examples: String,
}

/// Everything Settings → Servers shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServersSnapshot {
    pub servers: Vec<ServerView>,
    pub roles: Vec<ServerRole>,
    pub classes: Vec<CommandClassInfo>,
    /// Where keys and passwords are kept ("Windows Credential Manager").
    pub vault: VaultStatus,
    pub notices: Vec<String>,
    /// The owner's "Remote computers (SSH)" switch (Settings → Switches, ADR-023). Off: no
    /// worker connects to any server; the owner can still add and test servers.
    pub switched_on: bool,
}

/// A server's identity, read for the owner to check before pinning it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerIdentity {
    pub host: String,
    pub port: u16,
    /// "ssh-ed25519".
    pub algorithm: String,
    /// "SHA256:…".
    pub fingerprint: String,
}

/// The outcome of the owner's "Test the connection".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerTest {
    pub ok: bool,
    pub message: String,
}

// ---- The owner's terminal (Phase 12, ADR-031) --------------------------------------------------

/// Which shell the owner's terminal on this PC starts (Settings → Terminal). Never a path: Plenipo
/// finds each one itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TerminalShell {
    /// Windows PowerShell, part of Windows (the default).
    #[default]
    WindowsPowerShell,
    /// PowerShell 7, when it is installed.
    PowerShell7,
    /// Command Prompt.
    CommandPrompt,
}

/// A shell the owner can pick, and whether this PC has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShellOption {
    pub shell: TerminalShell,
    /// "Windows PowerShell".
    pub label: String,
    pub installed: bool,
    /// Where it is, when installed (shown in Diagnostics).
    #[ts(optional)]
    pub path: Option<String>,
}

/// Where a terminal opens: this PC, one of the owner's servers, or an AI tool's own sign-in or
/// sign-out (Phase 19, ADR-058). Only a place: never a program, a path, or a command line
/// (anything more is refused). An AI tool's place names the tool and the action; the command it
/// runs comes from that tool's adapter, from a fixed list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TerminalPlace {
    ThisPc,
    Server {
        server_id: String,
    },
    AiTool {
        runtime_id: String,
        action: plenipo_runtime::agent::AccountAction,
    },
}

impl<'de> Deserialize<'de> for TerminalPlace {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Raw {
            kind: String,
            server_id: Option<String>,
            runtime_id: Option<String>,
            action: Option<plenipo_runtime::agent::AccountAction>,
        }
        let raw = Raw::deserialize(d)?;
        match (raw.kind.as_str(), raw.server_id, raw.runtime_id, raw.action) {
            ("thisPc", None, None, None) => Ok(Self::ThisPc),
            ("server", Some(server_id), None, None) => Ok(Self::Server { server_id }),
            ("aiTool", None, Some(runtime_id), Some(action))
                if plenipo_runtime::agent::builtin_adapters()
                    .iter()
                    .any(|a| a.id() == runtime_id) =>
            {
                Ok(Self::AiTool { runtime_id, action })
            }
            _ => Err(serde::de::Error::custom(
                "a terminal opens on this PC, on a server, or for an AI tool's own sign-in",
            )),
        }
    }
}

/// An open terminal of the owner's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TerminalInfo {
    pub id: String,
    /// "This PC" or the server's name.
    pub title: String,
    pub place: TerminalPlace,
    /// The shell on this PC ("Windows PowerShell"), or who Plenipo signed in as on a server
    /// ("deploy@web01.example.com:22").
    pub detail: String,
    /// A server's kind (production servers are marked red).
    #[ts(optional)]
    pub environment: Option<plenipo_guard::Environment>,
    #[ts(type = "number")]
    pub opened_at: u64,
}

/// The terminal settings (Settings → Terminal) and the terminals open now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TerminalSettings {
    /// The shell a new terminal on this PC starts.
    pub shell: TerminalShell,
    pub shells: Vec<ShellOption>,
    /// Off Windows the choice does not apply: the shell used instead ("/bin/bash").
    #[ts(optional)]
    pub other_shell: Option<String>,
    /// The "Remote computers (SSH)" switch: while it is off, no terminal opens on a server.
    pub servers_switched_on: bool,
    pub open: Vec<TerminalInfo>,
}

/// What the owner's terminal sends to the screen, as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TerminalEvent {
    /// Output: the bytes the shell wrote, base64-encoded.
    Output { data: String },
    /// The terminal ended: why, in plain words, and the shell's exit code when it gave one.
    Ended {
        why: String,
        #[ts(optional)]
        code: Option<i32>,
    },
}

// ---- The task page (Phase 12) ------------------------------------------------------------------

/// A task's page: the pull requests, artifacts, and decisions of the task and every task under
/// it, and their approvals (waiting ones first).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskRecord {
    pub record: plenipo_ledger::WorkRecord,
    pub approvals: Vec<ApprovalView>,
}

// ---- The canvas's live view (Phase 18, ADR-053 §17–§19) -------------------------------------

/// Where a worker's work runs now, besides its AI company's cloud (where every model Plenipo
/// uses today does its thinking).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LivePlace {
    /// This PC: `what` is "a program", "Plenipo's browser", or "the screen".
    ThisPc { what: String },
    /// One of the owner's servers, by name (production servers are marked).
    Server { name: String, production: bool },
}

/// What a worker touched last in its task, from Guard's own record of its calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Touching {
    /// A folder inside its working copy ("src/pages"; "" is the project folder itself).
    Folder {
        #[ts(optional)]
        project: Option<String>,
        folder: String,
    },
    Server {
        name: String,
        production: bool,
    },
    Website {
        host: String,
    },
    /// The screen, mouse, and keyboard of this PC.
    Screen,
}

/// A worker in a step now, and where its work is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveWorker {
    pub grant_id: String,
    pub task_id: String,
    #[ts(optional)]
    pub position_id: Option<String>,
    pub worker: String,
    /// Its AI tool.
    pub runtime_id: String,
    #[ts(optional)]
    pub runs_on: Option<LivePlace>,
    #[ts(optional)]
    pub touching: Option<Touching>,
}

/// A hand-off moving between two members, for the canvas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveHandoff {
    pub id: String,
    /// `asked` (a request) or `answered` (its reply going back).
    pub kind: String,
    #[ts(optional)]
    pub from_position_id: Option<String>,
    #[ts(optional)]
    pub to_position_id: Option<String>,
    #[ts(type = "number")]
    pub at: u64,
}

/// The canvas's live view: who is working where, and hand-offs in the last minutes. Read from
/// what Plenipo already records; it invents nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveView {
    pub workers: Vec<LiveWorker>,
    pub handoffs: Vec<LiveHandoff>,
    #[ts(type = "number")]
    pub at: u64,
}

// ---- The owner's files (Phase 21, ADR-093) ----------------------------------------------------

/// What kind of place a file view's top folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileRootKind {
    /// The project's own folder.
    ProjectFolder,
    /// An objective's working copy of it (ADR-016).
    WorkingCopy,
}

/// The worker writing in a folder now (ADR-016's one writer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderWriter {
    /// Its title ("Senior Developer").
    pub worker: String,
    #[ts(optional)]
    pub position_id: Option<String>,
    /// Its conversation (Stop the worker stops its task there).
    pub session_id: String,
    pub task_id: String,
}

/// One of the file view's top folders: a project's folder, or one of its working copies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileRoot {
    /// `project:<project ID>` or `copy:<working copy ID>`.
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub kind: FileRootKind,
    /// "Project folder", or the working copy's branch.
    pub label: String,
    /// Where it is on this PC.
    pub path: String,
    /// The folder is there (a project folder can be moved or deleted outside Plenipo).
    pub exists: bool,
    /// The worker writing in it now.
    #[ts(optional)]
    pub writer: Option<FolderWriter>,
}

/// The folders Plenipo knows, for the file view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileRoots {
    pub roots: Vec<FileRoot>,
    /// A worker is using the screen, mouse, and keyboard: blocked files are hidden and nothing
    /// is saved until the owner takes over (ADR-093 §14).
    pub desktop_in_use: bool,
}

/// A file or a folder in a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderEntry {
    pub name: String,
    /// Its path inside the top folder, with `/`.
    pub path: String,
    pub folder: bool,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    #[ts(type = "number | null")]
    pub modified: Option<u64>,
    /// Workers may not touch it (Settings → Permissions → blocked files).
    pub blocked: bool,
}

/// One folder's contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderListing {
    pub root: String,
    pub path: String,
    pub entries: Vec<FolderEntry>,
    /// Entries past the most shown.
    pub more: u32,
}

/// How a text file's lines end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineEnding {
    /// Unix (`\n`).
    Lf,
    /// Windows (`\r\n`).
    Crlf,
}

/// What Plenipo shows of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(export)]
pub enum FileContent {
    /// Text, with `\n` between lines (the file's own line ending is kept when saving).
    Text {
        text: String,
        /// It starts with a byte-order mark (kept when saving).
        bom: bool,
        line_ending: LineEnding,
    },
    /// A picture, shown as it is.
    Picture { mime: String, data: String },
    /// Anything else: what it is, in words.
    Other { what: String },
}

/// Why a file opens read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(export)]
pub enum ReadOnlyWhy {
    /// A worker is writing in this working copy (or project folder) now.
    Writer { writer: FolderWriter },
    /// A worker is using the screen, mouse, and keyboard.
    Desktop,
    /// The file is marked read-only on the disk.
    Disk,
}

/// A file opened in Plenipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileView {
    pub root: String,
    pub path: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number | null")]
    pub modified: Option<u64>,
    /// Its contents' fingerprint (SHA-256), to tell whether it changed before a save.
    #[ts(optional)]
    pub hash: Option<String>,
    pub content: FileContent,
    /// A program or a script: Plenipo never starts it, and never opens it in another program.
    pub runs: bool,
    /// Workers may not touch it.
    pub blocked: bool,
    #[ts(optional)]
    pub read_only: Option<ReadOnlyWhy>,
}

/// What a save did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(export)]
pub enum SaveOutcome {
    Saved {
        hash: String,
        #[ts(type = "number")]
        size: u64,
        #[ts(type = "number | null")]
        modified: Option<u64>,
        added: u32,
        removed: u32,
    },
    /// The file changed on the disk (or is gone) since it was opened: nothing was written.
    ChangedOnDisk,
}

/// A file a worker is changing now (the file view marks it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangingFile {
    pub root: String,
    pub path: String,
    pub worker: String,
    #[ts(optional)]
    pub position_id: Option<String>,
}

/// A file the owner puts on an objective (Phase 21, ADR-093 §19–§21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ObjectiveFile {
    /// From the Files panel: a known top folder and the path inside it.
    File { root: String, path: String },
    /// Dropped from File Explorer: Plenipo's ticket for that drop, and which of its files.
    Dropped { drop: String, index: u32 },
}

/// Files dropped on Plenipo's window from File Explorer: Plenipo keeps where they are, and the
/// page gets only this (the `plenipo://drop` event).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DroppedFiles {
    /// Plenipo's ticket for the drop.
    pub drop: String,
    pub files: Vec<DroppedFile>,
    /// Where they were dropped, in the page's own pixels.
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DroppedFile {
    pub name: String,
    pub folder: bool,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
}
