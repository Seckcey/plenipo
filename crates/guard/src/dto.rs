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
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct PermissionSetInput {
    #[ts(optional)]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub levels: BTreeMap<Capability, Level>,
}

/// The owner's command lists. Each entry is a program name followed by arguments to match;
/// `*` as the last word matches any remaining arguments, and `*` inside a word matches any
/// characters, e.g. `cargo test *`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct CommandRules {
    /// Run without asking when the worker may run programs.
    pub approved: Vec<String>,
    /// Always ask, even when approved.
    pub ask: Vec<String>,
    /// Never run.
    pub blocked: Vec<String>,
    /// Approved, and given the named stored secrets without asking (ADR-048, secrets reach
    /// only the programs they are for). Any other run that would be given a stored secret asks
    /// the owner first. Missing in older documents: none.
    pub with_secrets: Vec<SecretRule>,
}

/// An approved command that may also be given named stored secrets without asking (ADR-048).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct SecretRule {
    /// The command, written like an approved command (`gh pr *`).
    pub rule: String,
    /// The names of the stored secrets it may be given.
    pub secrets: Vec<String>,
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
    /// Signing in to a website or app (Phase 10).
    SignIn,
    /// A worker taking control of the mouse and keyboard (Phase 10).
    DesktopControl,
}

impl SensitiveKind {
    pub const ALL: [Self; 11] = [
        Self::Production,
        Self::Dns,
        Self::Credentials,
        Self::Database,
        Self::CloudDelete,
        Self::Payment,
        Self::Outbound,
        Self::SignIn,
        Self::DesktopControl,
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
            Self::CloudDelete => "Deleting online: cloud resources, mail, files, messages",
            Self::Payment => "Money: buying, payments, refunds, payouts",
            Self::Outbound => "Sending or publishing outside this computer",
            Self::Privilege => "Running as administrator",
            Self::OutsideWorkspace => "Deleting or overwriting files outside the project folder",
            Self::SignIn => "Signing in to a website or app",
            Self::DesktopControl => "Taking control of your mouse and keyboard",
        }
    }

    /// Examples shown in Settings.
    pub fn examples(self) -> &'static str {
        match self {
            Self::Production => "vercel --prod, terraform apply, kubectl apply",
            Self::Dns => "aws route53, az network dns, Set-DnsServerResourceRecord",
            Self::Credentials => "passwd, gh auth, aws iam create-access-key, docker login",
            Self::Database => "DROP TABLE, TRUNCATE, redis-cli flushall, prisma migrate reset",
            Self::CloudDelete => {
                "aws … delete, az … delete, terraform destroy, kubectl delete; replacing or \
                 deleting a file through a Connection"
            }
            Self::Payment => {
                "stripe refunds create, payouts, charges; Buy now, Place order, Pay, or Checkout \
                 on a website; refunds and invoices through a Connection (these always ask)"
            }
            Self::Outbound => {
                "git push, npm publish, docker push, gh pr create, sending email; submitting a \
                 form or sending a message on a website; sending mail, a chat message, or an \
                 invitation through a Connection"
            }
            Self::Privilege => "sudo, runas, Start-Process -Verb RunAs",
            Self::OutsideWorkspace => "rm, del, or move with a path outside the project folder",
            Self::SignIn => "a Sign in or Log in button, a form with a password field",
            Self::DesktopControl => {
                "a worker starting to use the mouse and keyboard, and each click, typing, and \
                 key press after that (asked each time, with its reason)"
            }
        }
    }
}

/// What a sensitive action does: always asks (the default), or never runs. Only the website
/// switches (ADR-023) let three kinds go ahead without asking, on allowed websites.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SensitiveRule {
    #[default]
    Ask,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
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

/// The owner's on/off switches (ADR-023): whole features, and what workers may do on the
/// websites the owner allowed without asking first. The safety rules that keep the owner in
/// charge (no passwords or secrets, at most 3 tries at a CAPTCHA; the sign; Stop) have no
/// switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct Switches {
    /// Plenipo's browser. Off: no worker uses any website, whatever its permissions.
    pub browser: bool,
    /// The screen, mouse, and keyboard. Off (the default): no worker sees the screen or uses
    /// the mouse and keyboard.
    pub desktop: bool,
    /// On websites on the allowed list, submit forms and send messages without asking.
    pub send_without_asking: bool,
    /// On websites on the allowed list, buy and pay without asking.
    pub buy_without_asking: bool,
    /// On websites on the allowed list, sign in without asking (workers still never type a
    /// password).
    pub sign_in_without_asking: bool,
    /// When a website checks for a person (a CAPTCHA), the worker tries it at most 3 times, then
    /// hands it to the owner to solve and waits. Off: the worker stops there without trying
    /// (ADR-029).
    pub captcha_to_owner: bool,
    /// Keep a screenshot of every significant step in the Activity trail. Off: only approval
    /// cards keep a picture of the page.
    pub screenshots: bool,
    /// Remote computers over SSH (Phase 11, ADR-025). Off (the default): no worker connects to
    /// any server, whatever its permissions and the server's own settings.
    pub servers: bool,
    /// "Let workers use paid AI keys" (Phase 16 Wave 3, ADR-085). Off (the default): Plenipo
    /// uses only the owner's subscriptions, exactly as before; no paid key can be saved and no
    /// paid route is offered or run. On: workers may use the paid keys the owner saved, within
    /// the spending caps the owner set, if any (no cap is needed).
    pub paid_ai_keys: bool,
    /// "Let leads hire missing workers on their own" (Phase 25, item 2.7). Off (the default):
    /// when a lead needs a job nobody in its department does, Plenipo asks the owner first. On:
    /// it hires one on call for the lead's team, within Free's limits.
    pub hire_on_its_own: bool,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            browser: true,
            desktop: false,
            send_without_asking: false,
            buy_without_asking: false,
            sign_in_without_asking: false,
            captcha_to_owner: true,
            screenshots: true,
            servers: false,
            paid_ai_keys: false,
            hire_on_its_own: false,
        }
    }
}

impl Switches {
    /// The switch that lets `kind` go ahead without asking on an allowed website, if any.
    pub fn without_asking(&self, kind: SensitiveKind) -> Option<bool> {
        match kind {
            SensitiveKind::Outbound => Some(self.send_without_asking),
            SensitiveKind::Payment => Some(self.buy_without_asking),
            SensitiveKind::SignIn => Some(self.sign_in_without_asking),
            _ => None,
        }
    }
}

/// How much Plenipo asks before an agent saves files or runs programs (ADR-201, the owner's
/// order of 2026-10-03: "be light on restrictive permissions and let the user turn it up").
/// One setting for the whole organization, in Settings → Safety. It never gives an agent more
/// than its role's permission set; it only decides how much of that goes ahead without asking.
/// The rules that keep the owner in charge have no setting: secrets and blocked files, the
/// never-run list, deleting or overwriting outside the folder, running as administrator,
/// sending or publishing, and money.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Safety {
    /// The starting choice. Agents save files and run programs and scripts in their folder
    /// without asking, whether or not the program is on the approved list.
    #[default]
    Light,
    /// How earlier versions of Plenipo started: a program that is not on the approved list,
    /// and every PowerShell script, asks first.
    Careful,
    /// Agents only read. Nothing is saved, run, committed, or sent to GitHub.
    Strict,
}

impl Safety {
    /// The words for the audit trail and the Activity trail.
    pub fn words(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Careful => "Careful",
            Self::Strict => "Strict",
        }
    }
}

/// Which browser is Plenipo's browser (ADR-028). The owner chooses it in Settings → Permissions
/// → Websites; each browser keeps its own profile folder, so its sign-ins stay its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BrowserChoice {
    /// Microsoft Edge, or Google Chrome when Edge is not installed (as before ADR-028).
    #[default]
    Automatic,
    /// Microsoft Edge only.
    Edge,
    /// Google Chrome only (or Chromium, where Chrome itself is not made).
    Chrome,
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
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number")]
    pub updated_at: u64,
}

/// A secret as the owner submits it. `value`: the secret itself, sent once and stored only in
/// the operating system's protected storage; omit it to keep the stored value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
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
    /// Uses a website in Plenipo's browser (Phase 10).
    Web,
    /// Sees or uses this computer's screen, mouse, and keyboard (Phase 10).
    Screen,
    /// Runs commands on a server over SSH (Phase 11).
    Server,
}

impl Risk {
    pub fn words(self) -> &'static str {
        match self {
            Self::Read => "Looks at files",
            Self::Change => "Changes files",
            Self::Run => "Runs a program",
            Self::Delete => "Deletes files",
            Self::External => "Reaches outside this computer",
            Self::Web => "Uses a website",
            Self::Screen => "Uses your screen, mouse, or keyboard",
            Self::Server => "Runs commands on a server",
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
    /// Which websites workers may open in Plenipo's browser (Phase 10).
    pub websites: crate::websites::WebsiteRules,
    /// The owner's on/off switches (ADR-023).
    pub switches: Switches,
    /// How much Plenipo asks before an agent saves files or runs programs (ADR-201).
    pub safety: Safety,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paid_ai_keys_start_off_and_older_settings_read_as_off() {
        assert!(!Switches::default().paid_ai_keys);
        // A setting saved before Phase 16 Wave 3 has no such switch: it reads as off.
        let older: Switches =
            serde_json::from_value(serde_json::json!({ "browser": true, "servers": true }))
                .unwrap();
        assert!(!older.paid_ai_keys);
        assert!(older.servers);
        let on: Switches =
            serde_json::from_value(serde_json::json!({ "paidAiKeys": true })).unwrap();
        assert!(on.paid_ai_keys);
    }
}
