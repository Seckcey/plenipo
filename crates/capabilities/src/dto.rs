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

/// Where a terminal opens: this PC, or one of the owner's servers. Only a place: never a
/// program, a path, or a command line (anything more is refused).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TerminalPlace {
    ThisPc,
    Server { server_id: String },
}

impl<'de> Deserialize<'de> for TerminalPlace {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Raw {
            kind: String,
            server_id: Option<String>,
        }
        let raw = Raw::deserialize(d)?;
        match (raw.kind.as_str(), raw.server_id) {
            ("thisPc", None) => Ok(Self::ThisPc),
            ("server", Some(server_id)) => Ok(Self::Server { server_id }),
            _ => Err(serde::de::Error::custom(
                "a terminal opens on this PC or on a server",
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
