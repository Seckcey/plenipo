//! Guard DTOs shared with the frontend (camelCase on the wire).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::registry::Capability;

/// How far a permission goes. Ordered from strictest to most open: when layers disagree, the
/// strictest wins.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Level {
    /// Never.
    #[default]
    Blocked,
    /// Ask the owner each time.
    Ask,
    /// Without asking (other checks still apply).
    Allowed,
}

impl Level {
    pub fn words(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Ask => "ask me",
            Self::Allowed => "allowed",
        }
    }
}

/// A named set of permissions (the plan's capability profile). A role's set grants; a
/// project's or department's set only limits.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PermissionSet {
    /// A short slug, e.g. `developer` (also what a project stores as its limit).
    pub id: String,
    pub name: String,
    pub description: String,
    /// Capabilities not listed are blocked.
    #[ts(type = "Partial<Record<Capability, Level>>")]
    pub levels: BTreeMap<Capability, Level>,
    /// Shipped with Plenipo: can be changed but not removed.
    pub built_in: bool,
}

impl PermissionSet {
    pub fn level(&self, c: Capability) -> Level {
        self.levels.get(&c).copied().unwrap_or(Level::Blocked)
    }
}

/// A permission set as the owner submits it (no `id`: a new one).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PermissionSetInput {
    #[ts(optional)]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    #[ts(type = "Partial<Record<Capability, Level>>")]
    pub levels: BTreeMap<Capability, Level>,
}

/// The owner's command lists. Each entry is a program name followed by arguments to match;
/// `*` as the last word matches any remaining arguments, and `*` inside a word matches any
/// characters, e.g. `cargo test *`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CommandRules {
    /// Run without asking when the worker may run programs.
    pub approved: Vec<String>,
    /// Always ask, even when approved.
    pub ask: Vec<String>,
    /// Never run.
    pub blocked: Vec<String>,
}

/// Kinds of actions the plan says need the owner's approval by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SensitiveKind {
    Production,
    Dns,
    Credentials,
    Database,
    CloudDelete,
    Payment,
    Outbound,
    Privilege,
    OutsideWorkspace,
}

impl SensitiveKind {
    pub const ALL: [Self; 9] = [
        Self::Production,
        Self::Dns,
        Self::Credentials,
        Self::Database,
        Self::CloudDelete,
        Self::Payment,
        Self::Outbound,
        Self::Privilege,
        Self::OutsideWorkspace,
    ];

    /// Plain name, e.g. "Changing DNS".
    pub fn label(self) -> &'static str {
        match self {
            Self::Production => "Deploying or changing live systems",
            Self::Dns => "Changing DNS",
            Self::Credentials => "Changing passwords, keys, or other credentials",
            Self::Database => "Deleting or wiping database data",
            Self::CloudDelete => "Deleting cloud resources",
            Self::Payment => "Money: payments, refunds, payouts",
            Self::Outbound => "Sending or publishing outside this computer",
            Self::Privilege => "Running as administrator",
            Self::OutsideWorkspace => "Deleting or overwriting files outside the project folder",
        }
    }

    /// Examples shown in Settings.
    pub fn examples(self) -> &'static str {
        match self {
            Self::Production => "vercel --prod, terraform apply, kubectl apply",
            Self::Dns => "aws route53, az network dns, Set-DnsServerResourceRecord",
            Self::Credentials => "passwd, gh auth, aws iam create-access-key, docker login",
            Self::Database => "DROP TABLE, TRUNCATE, redis-cli flushall, prisma migrate reset",
            Self::CloudDelete => "aws … delete, az … delete, terraform destroy, kubectl delete",
            Self::Payment => "stripe refunds create, payouts, charges",
            Self::Outbound => "git push, npm publish, docker push, gh pr create, sending email",
            Self::Privilege => "sudo, runas, Start-Process -Verb RunAs",
            Self::OutsideWorkspace => "rm, del, or move with a path outside the project folder",
        }
    }
}

/// What a sensitive action does: always asks (the default), or never runs. It can never be
/// allowed without asking.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SensitiveRule {
    #[default]
    Ask,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GuardOptions {
    /// How long an approval waits for the owner before it expires.
    pub approval_minutes: u32,
}

pub const DEFAULT_APPROVAL_MINUTES: u32 = 10;
pub const MAX_APPROVAL_MINUTES: u32 = 60;

impl Default for GuardOptions {
    fn default() -> Self {
        Self {
            approval_minutes: DEFAULT_APPROVAL_MINUTES,
        }
    }
}

/// A secret kept in the operating system's protected storage. Only this reference is stored
/// by Plenipo; the value never is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SecretInfo {
    pub id: String,
    pub name: String,
    /// Environment variable the value is given as (to the programs below only).
    #[ts(optional)]
    pub env_var: Option<String>,
    /// Program names that receive it, e.g. `gh`.
    pub programs: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// A secret as the owner submits it. `value`: the secret itself, sent once and stored only in
/// the operating system's protected storage; omit it to keep the stored value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SecretInput {
    #[ts(optional)]
    pub id: Option<String>,
    pub name: String,
    #[ts(optional)]
    pub env_var: Option<String>,
    pub programs: Vec<String>,
    #[ts(optional)]
    pub value: Option<String>,
}

/// What kind of change an action makes, shown on approval cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Risk {
    /// Looks at files or history.
    Read,
    /// Changes files or the project's history.
    Change,
    /// Runs a program.
    Run,
    /// Deletes something.
    Delete,
    /// Reaches outside this computer.
    External,
}

impl Risk {
    pub fn words(self) -> &'static str {
        match self {
            Self::Read => "Looks at files",
            Self::Change => "Changes files",
            Self::Run => "Runs a program",
            Self::Delete => "Deletes files",
            Self::External => "Reaches outside this computer",
        }
    }
}

/// Guard's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Verdict {
    Allow,
    Ask,
    Deny,
}

/// Which check decided (or noted something).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Layer {
    Grant,
    Role,
    Project,
    Department,
    Target,
    Risk,
    Rule,
}

/// One layer's contribution to a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Check {
    pub layer: Layer,
    pub verdict: Verdict,
    pub note: String,
}

/// Guard's decision about one action, with a plain explanation and every layer's note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Decision {
    pub verdict: Verdict,
    /// One plain sentence: why.
    pub reason: String,
    /// The layer that decided.
    pub layer: Layer,
    pub risk: Risk,
    #[ts(optional)]
    pub sensitive: Option<SensitiveKind>,
    pub checks: Vec<Check>,
}

// ---- Settings views -------------------------------------------------------------------------

/// A capability as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapabilityInfo {
    pub id: Capability,
    pub label: String,
    pub description: String,
    /// Plenipo has tools for it in this version.
    pub tools: bool,
    /// When it has no tools yet: when they arrive ("Phase 10").
    #[ts(optional)]
    pub arrives: Option<String>,
}

/// A role and its permission set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RolePermissions {
    pub role_id: String,
    pub role_name: String,
    pub full_time: bool,
    #[ts(optional)]
    pub set_id: Option<String>,
}

/// A department's or project's limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UnitLimit {
    pub id: String,
    pub name: String,
    /// The permission set that limits its work (`None`: no limit).
    #[ts(optional)]
    pub set_id: Option<String>,
    /// Projects: the folder its workers work in.
    #[ts(optional)]
    pub folder: Option<String>,
    /// Something the owner should fix (a limit that is not a set, a missing folder).
    #[ts(optional)]
    pub problem: Option<String>,
}

/// A sensitive kind and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SensitiveInfo {
    pub kind: SensitiveKind,
    pub label: String,
    pub examples: String,
    pub rule: SensitiveRule,
}

/// Guard's settings, as Settings → Permissions shows them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GuardSettings {
    pub capabilities: Vec<CapabilityInfo>,
    pub sets: Vec<PermissionSet>,
    pub roles: Vec<RolePermissions>,
    pub departments: Vec<UnitLimit>,
    pub projects: Vec<UnitLimit>,
    pub commands: CommandRules,
    pub blocked_files: Vec<String>,
    pub sensitive: Vec<SensitiveInfo>,
    pub options: GuardOptions,
    pub secrets: Vec<SecretInfo>,
}
