//! Connections (Phase 20, ADR-062): the business's own accounts — Microsoft 365, Slack, and
//! Google so far — that Plenipo signs in to for the owner, and Guard's part of the decision about a
//! worker using one.
//!
//! A connection is set up by the owner in Settings → Connections: which of its parts are on, and
//! at which level (**Read only** or **Full access**); who may use it (roles and agents, each **Read
//! only** or **Read and write**); and the addresses it may send to without asking, while the
//! owner's switch allows that (ADR-023, widened by ADR-062 §5). Its sign-in token is never here:
//! only the Vault keeps it (ADR-063). Nothing here reaches a service: the capability broker does,
//! after asking Guard.
//!
//! The rules, in the order the engine applies them:
//! - the connection's own list grants (the agent's own line, else its role's); the project's and
//!   department's permission-set limits only narrow it (ADR-062 §3);
//! - the connection must be connected, and the part on — at **Full access** for anything but
//!   reading (the target);
//! - sending asks (unless the switch is on and every recipient is on the list), deleting asks, and
//!   paying always asks (the action's risk).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::dto::{Layer, SensitiveKind};
use crate::service::{Guard, OWNER, PLENIPO};

/// Most people and roles on a connection's **Who may use it** list, and most entries on its
/// **Send without asking to** list.
pub const MAX_ACCESS: usize = 200;
pub const MAX_SEND_LIST: usize = 200;
/// Most connections kept (Slack may have more than one, ADR-064 §3).
pub const MAX_CONNECTIONS: usize = 50;

/// A service Plenipo can connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Service {
    Microsoft365,
    Slack,
    Google,
    Hubspot,
    Stripe,
    Wordpress,
}

impl Service {
    /// In the order Settings shows them (the plan's order).
    pub const ALL: [Service; 6] = [
        Self::Microsoft365,
        Self::Slack,
        Self::Google,
        Self::Hubspot,
        Self::Stripe,
        Self::Wordpress,
    ];

    /// Its ID, which is also the ID of its first connection: `microsoft365`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Microsoft365 => "microsoft365",
            Self::Slack => "slack",
            Self::Google => "google",
            Self::Hubspot => "hubspot",
            Self::Stripe => "stripe",
            Self::Wordpress => "wordpress",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Its name on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Microsoft365 => "Microsoft 365",
            Self::Slack => "Slack",
            Self::Google => "Google",
            Self::Hubspot => "HubSpot",
            Self::Stripe => "Stripe",
            Self::Wordpress => "WordPress and WooCommerce",
        }
    }

    /// Built into this copy of Plenipo (Phase 20: Microsoft 365 in part 20A, Slack and Google in
    /// part 20B; ADR-067).
    pub fn built(self) -> bool {
        matches!(self, Self::Microsoft365 | Self::Slack | Self::Google)
    }

    /// Whether the owner may keep more than one account of it (Slack: any number of workspaces,
    /// ADR-064 §3); the others have one each.
    pub fn many(self) -> bool {
        self == Self::Slack
    }

    /// Its parts, in the order Settings shows them.
    pub fn parts(self) -> &'static [Part] {
        match self {
            Self::Microsoft365 => &[
                Part::Mail,
                Part::Calendar,
                Part::Onedrive,
                Part::Sharepoint,
                Part::Teams,
            ],
            Self::Slack => &[Part::Channels, Part::DirectMessages, Part::Search],
            Self::Google => &[Part::Gmail, Part::Calendar, Part::Drive],
            _ => &[],
        }
    }

    /// Whether `part` exists for an account of `kind` (personal Microsoft accounts have no Teams
    /// or SharePoint, ADR-065 §1). `None`: not signed in yet, so every part.
    pub fn has_part(self, part: Part, kind: Option<AccountKind>) -> bool {
        self.parts().contains(&part)
            && !(self == Self::Microsoft365
                && kind == Some(AccountKind::Personal)
                && matches!(part, Part::Sharepoint | Part::Teams))
    }

    /// A connection's parts when the owner first sets it up: Microsoft 365's Mail and Calendar,
    /// Slack's Channels, and Google's Gmail and Calendar at **Read only**; the rest off.
    pub fn starting_parts(self) -> BTreeMap<Part, PartLevel> {
        self.parts()
            .iter()
            .map(|p| {
                let level = match (self, p) {
                    (Self::Microsoft365, Part::Mail | Part::Calendar)
                    | (Self::Slack, Part::Channels)
                    | (Self::Google, Part::Gmail | Part::Calendar) => PartLevel::ReadOnly,
                    _ => PartLevel::Off,
                };
                (*p, level)
            })
            .collect()
    }
}

/// A part of a service (ADR-062 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Part {
    Mail,
    Calendar,
    Onedrive,
    Sharepoint,
    Teams,
    /// Slack's channels (and their threads).
    Channels,
    /// Slack's direct messages and group messages.
    DirectMessages,
    /// Searching Slack's messages.
    Search,
    Gmail,
    /// Google Drive.
    Drive,
}

impl Part {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mail => "Mail",
            Self::Calendar => "Calendar",
            Self::Onedrive => "OneDrive",
            Self::Sharepoint => "SharePoint",
            Self::Teams => "Teams",
            Self::Channels => "Channels",
            Self::DirectMessages => "Direct messages",
            Self::Search => "Search",
            Self::Gmail => "Gmail",
            Self::Drive => "Drive",
        }
    }

    /// Whether it has a **Full access** level: every part but Slack's Search, which only reads
    /// (ADR-069 §5.3).
    pub fn has_full_access(self) -> bool {
        self != Self::Search
    }
}

/// How far a part goes: off, reading only, or everything its tools can do (the owner's choices 5
/// and 8). Ordered from least to most.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PartLevel {
    #[default]
    Off,
    ReadOnly,
    FullAccess,
}

impl PartLevel {
    pub fn words(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ReadOnly => "Read only",
            Self::FullAccess => "Full access",
        }
    }
}

/// What kind of account a Microsoft 365 connection signed in with (the owner's choice 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AccountKind {
    /// A work or school account, in any organization.
    Work,
    /// A personal Microsoft account (outlook.com, hotmail.com).
    Personal,
}

/// Who a line on **Who may use it** is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Who {
    /// Every agent in this role.
    Role { id: String },
    /// One agent (a position), which wins over its role's line.
    Agent { id: String },
}

/// Read strictly: `{"kind": "role" | "agent", "id": "…"}` and nothing more.
impl<'de> Deserialize<'de> for Who {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            kind: String,
            id: String,
        }
        let raw = Raw::deserialize(d)?;
        match raw.kind.as_str() {
            "role" => Ok(Self::Role { id: raw.id }),
            "agent" => Ok(Self::Agent { id: raw.id }),
            _ => Err(serde::de::Error::custom("a line is for a role or an agent")),
        }
    }
}

/// How much a line on **Who may use it** allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AccessLevel {
    /// Reading tools only.
    ReadOnly,
    /// Reading and writing tools (sending, deleting, and paying still ask).
    ReadWrite,
}

impl AccessLevel {
    pub fn words(self) -> &'static str {
        match self {
            Self::ReadOnly => "Read only",
            Self::ReadWrite => "Read and write",
        }
    }
}

/// One line on **Who may use it**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Access {
    pub who: Who,
    pub level: AccessLevel,
}

/// Who the connection is signed in as, as the service reported it (never a token).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Account {
    /// "Frankie Gonzalez".
    pub name: String,
    /// "frankie@8westit.com".
    pub address: String,
    /// The organization it belongs to, when known.
    #[ts(optional)]
    pub organization: Option<String>,
    /// The organization's ID at the service (Microsoft's tenant ID), where its sign-in is kept
    /// fresh. Not a secret.
    #[ts(optional)]
    pub tenant: Option<String>,
}

/// Where a connection stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectionState {
    #[default]
    NotConnected,
    Connected,
    /// The service refused its sign-in (expired, removed, password changed): the owner signs in
    /// again.
    NeedsSignIn,
}

/// The owner's own app, used instead of 8 West's: an organization's own Microsoft app (choice 4),
/// a Slack workspace's own app, or the owner's Google app (ADR-069 §3–§4). Not a secret: a Google
/// app's secret is kept only in the Vault, and this says only that it is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OwnApp {
    /// The app's ID: Microsoft's "Application (client) ID" (a GUID), or Slack's or Google's
    /// client ID.
    pub app_id: String,
    /// Microsoft 365 only: the organization it is registered in, its domain (`contoso.com`) or
    /// its ID (a GUID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tenant: Option<String>,
    /// Google only: its secret is kept in the Vault (never here).
    #[serde(default)]
    pub secret_kept: bool,
}

/// One connection, as Guard's settings keep it. Its sign-in is only in the Vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Connection {
    /// `microsoft365` (a service's first connection has the service's ID).
    pub id: String,
    pub service: Service,
    /// What kind of account it signed in with (`None`: not connected yet).
    #[ts(optional)]
    pub account_kind: Option<AccountKind>,
    #[ts(optional)]
    pub account: Option<Account>,
    /// Each part's level; a part not listed is off.
    pub parts: BTreeMap<Part, PartLevel>,
    /// The permissions the service granted at the last sign-in, in its own words.
    pub granted: Vec<String>,
    /// **Who may use it**: empty until the owner picks.
    pub access: Vec<Access>,
    /// **Send without asking to**: addresses, `@domains`, and channels.
    pub send_list: Vec<String>,
    /// The organization's own app, instead of 8 West's (Advanced).
    #[ts(optional)]
    pub own_app: Option<OwnApp>,
    #[ts(optional, type = "number")]
    pub connected_at: Option<u64>,
    pub state: ConnectionState,
}

impl Connection {
    /// A service's connection before the owner changed anything.
    pub fn new(id: &str, service: Service) -> Self {
        Self {
            id: id.into(),
            service,
            account_kind: None,
            account: None,
            parts: service.starting_parts(),
            granted: Vec::new(),
            access: Vec::new(),
            send_list: Vec::new(),
            own_app: None,
            connected_at: None,
            state: ConnectionState::NotConnected,
        }
    }

    /// Its name on screen: "Microsoft 365".
    pub fn label(&self) -> &'static str {
        self.service.label()
    }

    /// A part's level, as far as the account allows.
    pub fn part(&self, part: Part) -> PartLevel {
        if !self.service.has_part(part, self.account_kind) {
            return PartLevel::Off;
        }
        self.parts.get(&part).copied().unwrap_or_default()
    }

    /// The line on **Who may use it** that applies to an agent in a role: the agent's own, else
    /// its role's.
    pub fn line_for(&self, position_id: Option<&str>, role_id: &str) -> Option<&Access> {
        position_id
            .and_then(|p| {
                self.access
                    .iter()
                    .find(|a| matches!(&a.who, Who::Agent { id } if id == p))
            })
            .or_else(|| {
                self.access
                    .iter()
                    .find(|a| matches!(&a.who, Who::Role { id } if id == role_id))
            })
    }
}

/// Whether a connection ID can belong to `service`: the service's own ID, or (Slack, which may
/// have more than one, ADR-064 §3) the service's ID and a number from 2 to 999, `slack-2`.
pub fn id_belongs_to(id: &str, service: Service) -> bool {
    id == service.id()
        || (service == Service::Slack
            && id.strip_prefix("slack-").is_some_and(|n| {
                !n.starts_with('0')
                    && n.len() <= 3
                    && n.chars().all(|c| c.is_ascii_digit())
                    && n.parse::<u16>().is_ok_and(|v| v >= 2)
            }))
}

/// The service a connection ID belongs to.
pub fn service_of(id: &str) -> Option<Service> {
    Service::ALL.into_iter().find(|s| id_belongs_to(id, *s))
}

// ---- Checking what the owner types ------------------------------------------------------------

/// A GUID, as Microsoft writes an app's or an organization's ID.
pub fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// A domain name: letters, digits, and dashes in dot-separated labels, with at least one dot.
pub fn is_domain(s: &str) -> bool {
    s.len() <= 253
        && s.contains('.')
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// A Slack channel's ID (`C0123ABCD`, or `G…` for older private channels): fixed for the
/// channel's life and never reused, so it can be on a **Send without asking to** list
/// (ADR-069 §1). Direct messages (`D…`) cannot: their people decide.
pub fn is_slack_channel(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some('C' | 'G'))
        && (9..=13).contains(&s.len())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// A Slack app's client ID: two numbers joined by a dot (`1234567890.9876543210`).
pub fn is_slack_client_id(s: &str) -> bool {
    s.len() <= 60
        && s.split_once('.').is_some_and(|(a, b)| {
            !a.is_empty()
                && !b.is_empty()
                && (a.chars().chain(b.chars())).all(|c| c.is_ascii_digit())
        })
}

/// A Google app's client ID: `<digits>-<letters and digits>.apps.googleusercontent.com`.
pub fn is_google_client_id(s: &str) -> bool {
    s.len() <= 200
        && s.strip_suffix(".apps.googleusercontent.com")
            .and_then(|head| head.split_once('-'))
            .is_some_and(|(n, rest)| {
                !n.is_empty()
                    && n.chars().all(|c| c.is_ascii_digit())
                    && !rest.is_empty()
                    && rest.chars().all(|c| c.is_ascii_alphanumeric())
            })
}

/// An email address, simply: `name@domain`.
pub fn is_address(s: &str) -> bool {
    let Some((name, domain)) = s.rsplit_once('@') else {
        return false;
    };
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ADDRESS_MARKS.contains(c))
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.contains("..")
        && is_domain(domain)
}

/// The marks an address may have before its `@` besides letters and digits (RFC 5322's
/// "atext" and the dot): `dana=40x.com@lists.org` and `bounce+id@mail.co` are real addresses.
/// Never a space, comma, quotation mark, angle bracket, or line break.
const ADDRESS_MARKS: &str = ".!#$%&'*+-/=?^_`{|}~";

/// One entry of a `service`'s **Send without asking to** list, as kept: an address or an
/// `@domain`, in lower case, or — Slack only — a channel's ID, in capitals. `Err`: why it is not
/// one. A Teams channel cannot be on a list: it is known only by names anyone can reuse, so
/// posting in one always asks (as built, part 20A). A Slack channel can, by its ID, which never
/// changes and is never reused (ADR-069 §1).
pub fn send_entry(entry: &str, service: Service) -> Result<String, String> {
    let e = entry.trim();
    if e.is_empty() || e.chars().count() > 200 || e.chars().any(char::is_control) {
        return Err("each entry must be one line of 1–200 characters".into());
    }
    // As Slack shows it, in capitals: a channel's name is in small letters, so "companynews"
    // is a name, never taken for an ID.
    if service == Service::Slack && is_slack_channel(e) {
        return Ok(e.to_owned());
    }
    let lower = e.to_lowercase();
    if let Some(domain) = lower.strip_prefix('@') {
        return if is_domain(domain) {
            Ok(lower)
        } else {
            Err(format!(
                "{e:?} is not a domain (write it like @8westit.com)"
            ))
        };
    }
    if lower.contains('@') {
        return if is_address(&lower) {
            Ok(lower)
        } else {
            Err(format!("{e:?} is not an email address"))
        };
    }
    if service == Service::Slack {
        return Err(format!(
            "{e:?} is not an email address, an @domain, or a Slack channel's ID (write it like \
             dana@clientco.com, @clientco.com, or C0123ABCD in capitals, as Slack shows it: in \
             Slack, click the channel's name; its ID is at the bottom of About)"
        ));
    }
    if e.starts_with('#') || e.contains('›') {
        return Err(format!(
            "{e:?} is a channel: posting in a channel always asks you, so it cannot be on this list"
        ));
    }
    Err(format!(
        "{e:?} is not an email address or an @domain (write it like dana@clientco.com or \
         @clientco.com)"
    ))
}

/// Whether `recipient` of a send through a `service` connection is on `list`: a real email address
/// (as [`listed`]), or — Slack only — a channel's ID that is on the list exactly.
pub fn listed_for(service: Service, list: &[String], recipient: &str) -> bool {
    let r = recipient.trim();
    if service == Service::Slack && is_slack_channel(r) {
        return list.iter().any(|e| e == r);
    }
    listed(list, r)
}

/// Whether `recipient` is on `list`. Only a real email address can be: an address entry matches
/// it exactly, and an `@domain` entry matches its domain exactly (never a subdomain). Anything
/// else — a person known only by a name anyone can set, a channel — is never on the list, so a
/// send to it asks the owner.
pub fn listed(list: &[String], recipient: &str) -> bool {
    let r = recipient.trim().to_lowercase();
    if !is_address(&r) {
        return false;
    }
    let domain = r.rsplit_once('@').map(|(_, d)| d);
    list.iter().any(|entry| {
        let e = entry.trim().to_lowercase();
        match e.strip_prefix('@') {
            Some(d) => domain == Some(d),
            None => e == r,
        }
    })
}

// ---- Guard's decision -------------------------------------------------------------------------

/// What a connection tool does (ADR-062 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolKind {
    /// Reads: mail, events, files, messages.
    Read,
    /// Writes something that stays in the owner's account: a draft, an event with no guests, a
    /// new file.
    Write,
    /// Sends or posts to people: a draft sent, a chat message, invitations.
    Send,
    /// Deletes or replaces something.
    Delete,
    /// Moves money.
    Pay,
}

impl ToolKind {
    /// Whether it needs a part at **Full access** and a worker at **Read and write**.
    pub fn writes(self) -> bool {
        self != Self::Read
    }

    /// The sensitive kind it is, and why in a clause.
    pub fn sensitive(self) -> Option<(SensitiveKind, &'static str)> {
        match self {
            Self::Read | Self::Write => None,
            Self::Send => Some((
                SensitiveKind::Outbound,
                "it sends or posts to people through a Connection",
            )),
            Self::Delete => Some((
                SensitiveKind::CloudDelete,
                "it deletes or replaces something through a Connection",
            )),
            Self::Pay => Some((
                SensitiveKind::Payment,
                "it moves money through a Connection",
            )),
        }
    }
}

/// A worker's use of a connection, for Guard.
#[derive(Debug, Clone, Copy)]
pub struct ConnectionCheck<'a> {
    pub connection: &'a Connection,
    pub part: Part,
    pub kind: ToolKind,
    /// Everyone a send reaches, as the service has them (addresses; channel names).
    pub recipients: &'a [String],
}

/// Guard's answer about the connection part of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionVerdict {
    /// Never, and why (a clause after "Blocked: ").
    Deny { layer: Layer, reason: String },
    /// Allowed as far as the connection goes. `sensitive`: the kind of action it is, if any;
    /// `all_listed`: a send whose every recipient is on the connection's **Send without asking
    /// to** list.
    Go {
        sensitive: Option<(SensitiveKind, &'static str)>,
        all_listed: bool,
    },
}

fn deny(layer: Layer, reason: String) -> ConnectionVerdict {
    ConnectionVerdict::Deny { layer, reason }
}

/// Check a worker's use of a connection against the owner's settings for it: connected, and the
/// part on at a level that allows the tool.
pub fn check(c: &ConnectionCheck<'_>) -> ConnectionVerdict {
    let conn = c.connection;
    let name = conn.label();
    match conn.state {
        ConnectionState::NotConnected => {
            return deny(Layer::Target, format!("{name} is not connected"));
        }
        ConnectionState::NeedsSignIn => {
            return deny(
                Layer::Target,
                format!("{name} needs the owner to sign in again (Settings → Connections)"),
            );
        }
        ConnectionState::Connected => {}
    }
    if !conn.service.has_part(c.part, conn.account_kind) {
        return deny(
            Layer::Target,
            format!("{name} has no {} for a personal account", c.part.label()),
        );
    }
    match conn.part(c.part) {
        PartLevel::Off => {
            return deny(
                Layer::Rule,
                format!(
                    "{name}'s {} is off (Settings → Connections)",
                    c.part.label()
                ),
            )
        }
        PartLevel::ReadOnly if c.kind.writes() => {
            return deny(
                Layer::Rule,
                format!(
                    "{name}'s {} is Read only (Settings → Connections)",
                    c.part.label()
                ),
            )
        }
        _ => {}
    }
    let all_listed = c.kind == ToolKind::Send
        && !c.recipients.is_empty()
        && c.recipients
            .iter()
            .all(|r| listed_for(conn.service, &conn.send_list, r));
    ConnectionVerdict::Go {
        sensitive: c.kind.sensitive(),
        all_listed,
    }
}

// ---- The owner's actions ----------------------------------------------------------------------

/// What the owner asked for on a connection's card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionAction {
    Connect,
    Cancel,
    Disconnect,
    /// Change its parts, who may use it, its list, or its app.
    Change,
}

impl ConnectionAction {
    /// The word in the record: `connect`, `cancel`, `disconnect`, `change`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Cancel => "cancel",
            Self::Disconnect => "disconnect",
            Self::Change => "change",
        }
    }
}

/// What Guard needs to know to decide about the owner's action on a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRequest<'a> {
    pub connection_id: &'a str,
    pub action: ConnectionAction,
    /// A sign-in for it is already waiting in the owner's browser.
    pub signing_in: bool,
    /// This copy of Plenipo has an app ID to sign in with (8 West's, or the organization's own).
    pub has_app: bool,
    /// For Connect: at least one part is on.
    pub parts_on: bool,
}

impl Guard {
    /// Check the owner's action on a connection. A refusal says why in plain words and is
    /// recorded (`guard.connection_refused`). Disconnecting is always allowed for a known
    /// connection: taking back a sign-in is never locked (ADR-068).
    pub fn check_connection_action(&self, request: &ConnectionRequest<'_>) -> Result<(), String> {
        decide(request).inspect_err(|why| {
            let event = plenipo_ledger::NewEvent {
                source: PLENIPO.into(),
                event_type: "guard.connection_refused".into(),
                payload: json!({
                    "connectionId": request.connection_id,
                    "action": request.action.as_str(),
                    "reason": why,
                }),
                ..plenipo_ledger::NewEvent::default()
            };
            let _ = self.ledger().append_event(event);
        })
    }
}

/// What a connection change records (never a token, never an account's address).
fn change_payload(c: &Connection, what: &str) -> serde_json::Value {
    json!({
        "connectionId": c.id,
        "service": c.service.label(),
        "what": what,
        "parts": c.parts.iter().map(|(p, l)| json!({ "part": p.label(), "level": l.words() })).collect::<Vec<_>>(),
        "access": c.access.iter().map(|a| json!({ "who": a.who, "level": a.level.words() })).collect::<Vec<_>>(),
        "sendList": c.send_list,
        "ownApp": c.own_app.as_ref().map(|a| &a.app_id),
    })
}

impl Guard {
    /// Every connection kept (not the services never set up).
    pub fn connections(&self) -> crate::Result<Vec<Connection>> {
        Ok(self.config()?.connections)
    }

    /// The connection `id`, as kept, or as it starts for a known service.
    pub fn connection(&self, id: &str) -> crate::Result<Connection> {
        self.config()?.connection_or_new(id)
    }

    /// Set the levels of a connection's parts. Records `connection.changed`.
    pub fn set_connection_parts(
        &self,
        id: &str,
        parts: &BTreeMap<Part, PartLevel>,
    ) -> crate::Result<Connection> {
        self.update("connection.changed", OWNER, |c| {
            let conn = c.set_connection_parts(id, parts)?;
            Ok(Some((change_payload(&conn, "parts"), conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Set a connection's **Who may use it** list. Records `connection.changed`.
    pub fn set_connection_access(&self, id: &str, access: &[Access]) -> crate::Result<Connection> {
        let records = self.ledger().org_records()?;
        let roles: Vec<String> = records.roles.iter().map(|r| r.id.clone()).collect();
        let agents: Vec<String> = records
            .positions
            .iter()
            .filter(|p| !p.is_deleted())
            .map(|p| p.id.clone())
            .collect();
        self.update("connection.changed", OWNER, |c| {
            let conn = c.set_connection_access(id, access, &roles, &agents)?;
            Ok(Some((change_payload(&conn, "access"), conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Set a connection's **Send without asking to** list. Records `connection.changed`.
    pub fn set_connection_send_list(&self, id: &str, list: &[String]) -> crate::Result<Connection> {
        self.update("connection.changed", OWNER, |c| {
            let conn = c.set_connection_send_list(id, list)?;
            Ok(Some((change_payload(&conn, "sendList"), conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Use an organization's own app, or 8 West's (`None`). Records `connection.changed`.
    pub fn set_connection_own_app(
        &self,
        id: &str,
        app: Option<&OwnApp>,
    ) -> crate::Result<Connection> {
        self.update("connection.changed", OWNER, |c| {
            let conn = c.set_connection_own_app(id, app)?;
            Ok(Some((change_payload(&conn, "app"), conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Use the owner's own Slack or Google app, or none (ADR-069 §3–§4). Records
    /// `connection.changed` (the client ID, never a secret).
    pub fn set_connection_client_app(
        &self,
        id: &str,
        client_id: Option<&str>,
        secret_kept: bool,
    ) -> crate::Result<Connection> {
        self.update("connection.changed", OWNER, |c| {
            let conn = c.set_connection_client_app(id, client_id, secret_kept)?;
            Ok(Some((change_payload(&conn, "app"), conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Add another account of `service` (Slack's workspaces). Records `connection.changed`.
    pub fn add_connection(&self, service: Service) -> crate::Result<Connection> {
        self.update("connection.changed", OWNER, |c| {
            let conn = c.add_connection(service)?;
            Ok(Some((change_payload(&conn, "added"), conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// Remove a card that is not connected. Records `connection.removed`.
    pub fn remove_connection(&self, id: &str) -> crate::Result<Connection> {
        self.update("connection.removed", OWNER, |c| {
            let conn = c.remove_connection(id)?;
            Ok(Some((
                json!({ "connectionId": conn.id, "service": conn.service.label() }),
                conn,
            )))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// A sign-in finished. Records `connection.connected` with the parts and what was granted —
    /// never the token or the account's address.
    pub fn connection_connected(
        &self,
        id: &str,
        kind: Option<AccountKind>,
        account: Account,
        granted: &[String],
    ) -> crate::Result<Connection> {
        let now = plenipo_ledger::now_ms();
        self.update("connection.connected", OWNER, |c| {
            let conn = c.connection_connected(id, kind, account, granted, now)?;
            let payload = json!({
                "connectionId": conn.id,
                "service": conn.service.label(),
                "accountKind": conn.account_kind,
                "parts": change_payload(&conn, "")["parts"],
                "granted": conn.granted,
            });
            Ok(Some((payload, conn)))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }

    /// The service refused the connection's sign-in. Records `connection.sign_in_needed` with
    /// the reason (in plain words, never a token). `None`: it was not connected.
    pub fn connection_needs_sign_in(
        &self,
        id: &str,
        reason: &str,
    ) -> crate::Result<Option<Connection>> {
        self.update("connection.sign_in_needed", PLENIPO, |c| {
            Ok(c.connection_needs_sign_in(id)?.map(|conn| {
                (
                    json!({ "connectionId": conn.id, "service": conn.service.label(), "reason": reason }),
                    conn,
                )
            }))
        })
    }

    /// Disconnected (the Vault's values are removed by the broker). Records
    /// `connection.disconnected`.
    pub fn connection_disconnected(&self, id: &str) -> crate::Result<Connection> {
        self.update("connection.disconnected", OWNER, |c| {
            let conn = c.connection_disconnected(id)?;
            Ok(Some((
                json!({ "connectionId": conn.id, "service": conn.service.label() }),
                conn,
            )))
        })?
        .ok_or_else(|| crate::GuardError::Invalid("nothing changed".into()))
    }
}

fn decide(request: &ConnectionRequest<'_>) -> Result<(), String> {
    let Some(service) = service_of(request.connection_id) else {
        return Err(format!(
            "Plenipo has no connection called {:?}.",
            request.connection_id
        ));
    };
    let name = service.label();
    if request.action == ConnectionAction::Disconnect {
        return Ok(());
    }
    if !service.built() {
        return Err(format!("{name} comes in a later update of Plenipo."));
    }
    match request.action {
        ConnectionAction::Connect => {
            if request.signing_in {
                return Err(format!(
                    "A sign-in to {name} is already waiting in your browser. Finish it, or press \
                     Cancel first."
                ));
            }
            if !request.has_app {
                return Err(format!(
                    "This copy of Plenipo has no app ID for {name} yet, so it cannot sign in."
                ));
            }
            if !request.parts_on {
                return Err(format!(
                    "Turn on at least one part of {name} first, so Plenipo knows what to ask for."
                ));
            }
            Ok(())
        }
        ConnectionAction::Cancel if !request.signing_in => {
            Err(format!("No sign-in to {name} is waiting."))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> Connection {
        let mut c = Connection::new("microsoft365", Service::Microsoft365);
        c.state = ConnectionState::Connected;
        c.account_kind = Some(AccountKind::Work);
        c.parts.insert(Part::Mail, PartLevel::FullAccess);
        c.parts.insert(Part::Teams, PartLevel::ReadOnly);
        c
    }

    fn verdict(c: &Connection, part: Part, kind: ToolKind, to: &[&str]) -> ConnectionVerdict {
        let to: Vec<String> = to.iter().map(|s| (*s).to_owned()).collect();
        check(&ConnectionCheck {
            connection: c,
            part,
            kind,
            recipients: &to,
        })
    }

    #[test]
    fn a_connection_must_be_connected_with_the_part_on_at_its_level() {
        let mut c = connected();
        assert!(matches!(
            verdict(&c, Part::Mail, ToolKind::Read, &[]),
            ConnectionVerdict::Go {
                sensitive: None,
                ..
            }
        ));
        // Calendar starts at Read only: reading goes, writing does not.
        assert!(matches!(
            verdict(&c, Part::Calendar, ToolKind::Read, &[]),
            ConnectionVerdict::Go { .. }
        ));
        let ConnectionVerdict::Deny { reason, .. } =
            verdict(&c, Part::Calendar, ToolKind::Write, &[])
        else {
            panic!("a Read only part cannot write");
        };
        assert!(reason.contains("Calendar is Read only"), "{reason}");
        let ConnectionVerdict::Deny { reason, .. } =
            verdict(&c, Part::Teams, ToolKind::Send, &["a@b.co"])
        else {
            panic!("a Read only part cannot send");
        };
        assert!(reason.contains("Teams is Read only"), "{reason}");
        let ConnectionVerdict::Deny { reason, .. } =
            verdict(&c, Part::Onedrive, ToolKind::Read, &[])
        else {
            panic!("an off part offers nothing");
        };
        assert!(reason.contains("OneDrive is off"), "{reason}");
        c.state = ConnectionState::NeedsSignIn;
        let ConnectionVerdict::Deny { reason, .. } = verdict(&c, Part::Mail, ToolKind::Read, &[])
        else {
            panic!("needs a sign-in");
        };
        assert!(reason.contains("sign in again"), "{reason}");
        c.state = ConnectionState::NotConnected;
        assert!(matches!(
            verdict(&c, Part::Mail, ToolKind::Read, &[]),
            ConnectionVerdict::Deny { .. }
        ));
    }

    #[test]
    fn personal_accounts_have_no_teams_or_sharepoint() {
        let mut c = connected();
        c.account_kind = Some(AccountKind::Personal);
        c.parts.insert(Part::Teams, PartLevel::FullAccess);
        assert_eq!(c.part(Part::Teams), PartLevel::Off);
        assert!(matches!(
            verdict(&c, Part::Teams, ToolKind::Read, &[]),
            ConnectionVerdict::Deny { .. }
        ));
        assert!(matches!(
            verdict(&c, Part::Mail, ToolKind::Read, &[]),
            ConnectionVerdict::Go { .. }
        ));
    }

    #[test]
    fn sending_deleting_and_paying_are_sensitive_and_listed_recipients_are_found() {
        let mut c = connected();
        c.send_list = vec!["@8westit.com".into(), "client@example.com".into()];
        let ConnectionVerdict::Go {
            sensitive,
            all_listed,
        } = verdict(
            &c,
            Part::Mail,
            ToolKind::Send,
            &["Frankie@8WestIT.com", "client@example.com"],
        )
        else {
            panic!()
        };
        assert_eq!(sensitive.map(|s| s.0), Some(SensitiveKind::Outbound));
        assert!(all_listed);
        // One recipient off the list: it asks.
        let ConnectionVerdict::Go { all_listed, .. } = verdict(
            &c,
            Part::Mail,
            ToolKind::Send,
            &["client@example.com", "attacker@evil.test"],
        ) else {
            panic!()
        };
        assert!(!all_listed);
        // A subdomain is not the domain, nor a look-alike.
        assert!(!listed(&c.send_list, "x@mail.8westit.com"));
        assert!(!listed(&c.send_list, "x@evil-8westit.com"));
        assert!(!listed(&c.send_list, "x@8westit.com.evil.test"));
        // Only a real address: a name with an address in it, a person known only by a name, or
        // a channel is never on the list.
        assert!(listed(&c.send_list, " Dana@8WestIT.com "));
        for not_one in [
            "Dana <dana@8westit.com>",
            "ceo@8westit.com (no email address in Teams)",
            "8 West IT › General",
            "@8westit.com",
            "",
        ] {
            assert!(!listed(&c.send_list, not_one), "{not_one:?}");
        }
        // No recipients: never "all listed".
        let ConnectionVerdict::Go { all_listed, .. } = verdict(&c, Part::Mail, ToolKind::Send, &[])
        else {
            panic!()
        };
        assert!(!all_listed);
        assert_eq!(
            ToolKind::Delete.sensitive().map(|s| s.0),
            Some(SensitiveKind::CloudDelete)
        );
        assert_eq!(
            ToolKind::Pay.sensitive().map(|s| s.0),
            Some(SensitiveKind::Payment)
        );
    }

    #[test]
    fn list_entries_are_addresses_or_domains_never_channels() {
        let m = Service::Microsoft365;
        assert_eq!(send_entry(" @8WestIT.com ", m).unwrap(), "@8westit.com");
        assert_eq!(
            send_entry("Client@Example.com", m).unwrap(),
            "client@example.com"
        );
        // Channels: posting in one always asks (Microsoft 365 and Google).
        for channel in ["Sales › General", "#general"] {
            assert!(
                send_entry(channel, m).unwrap_err().contains("always asks"),
                "{channel}"
            );
        }
        assert!(send_entry("C0100000001", m).is_err());
        assert!(send_entry("C0100000001", Service::Google).is_err());
        for bad in [
            "",
            "@",
            "@nodot",
            "bob@",
            "not an address",
            "a\nb",
            "@-x.com",
            "a,b@x.com",
            "\"a\"@x.com",
            "<a@x.com>",
            ".a@x.com",
            "a..b@x.com",
        ] {
            assert!(send_entry(bad, m).is_err(), "{bad:?}");
        }
        // Real addresses with marks before the @ (a mailing list's, a bounce address).
        for good in [
            "dana=40clientco.com@lists.org",
            "bounce+x&y@mail.co",
            "o'neil@x.com",
        ] {
            assert_eq!(send_entry(good, m).unwrap(), good);
            assert!(is_address(good), "{good}");
        }
    }

    #[test]
    fn a_slack_channel_is_on_a_slack_list_by_its_id_only() {
        let s = Service::Slack;
        assert_eq!(send_entry(" C0100000001 ", s).unwrap(), "C0100000001");
        assert_eq!(send_entry("G0400000004", s).unwrap(), "G0400000004");
        assert_eq!(
            send_entry("dana@ClientCo.com", s).unwrap(),
            "dana@clientco.com"
        );
        // A name, a direct message's ID, or something close is not a channel ID. A name in
        // small letters that is shaped like an ID is still a name.
        for bad in [
            "#general",
            "general",
            "companynews",
            "c0100000001",
            "D0300000003",
            "C01",
            "C0100000001X9Z",
            "C01-0000001",
        ] {
            let err = send_entry(bad, s).unwrap_err();
            assert!(err.contains("channel's ID"), "{bad}: {err}");
        }
        let list = vec!["C0100000001".to_owned(), "@8westit.com".to_owned()];
        assert!(listed_for(s, &list, "C0100000001"));
        assert!(listed_for(s, &list, "frankie@8westit.com"));
        // Another channel, a name that looks like the ID, or a lower-case copy: not listed.
        assert!(!listed_for(s, &list, "C0200000002"));
        assert!(!listed_for(s, &list, "c0100000001"));
        assert!(!listed_for(s, &list, "#general (C0100000001)"));
        // A channel ID never counts for Microsoft 365 or Google.
        assert!(!listed_for(Service::Microsoft365, &list, "C0100000001"));
        assert!(!listed_for(Service::Google, &list, "C0100000001"));
        // A Slack send to a listed channel goes without asking only with the switch (the engine).
        let mut c = Connection::new("slack", s);
        c.state = ConnectionState::Connected;
        c.parts.insert(Part::Channels, PartLevel::FullAccess);
        c.send_list = list;
        let ConnectionVerdict::Go { all_listed, .. } =
            verdict(&c, Part::Channels, ToolKind::Send, &["C0100000001"])
        else {
            panic!()
        };
        assert!(all_listed);
        let ConnectionVerdict::Go { all_listed, .. } = verdict(
            &c,
            Part::Channels,
            ToolKind::Send,
            &["dana (no email address in Slack)"],
        ) else {
            panic!()
        };
        assert!(!all_listed);
    }

    #[test]
    fn app_ids_and_parts_of_slack_and_google() {
        assert!(is_slack_client_id("1234567890.9876543210"));
        for bad in ["", "1234567890", ".1", "1.", "1.2.3", "a.b", "12 34.5"] {
            assert!(!is_slack_client_id(bad), "{bad:?}");
        }
        assert!(is_google_client_id(
            "123456789012-abcdef123.apps.googleusercontent.com"
        ));
        for bad in [
            "",
            "abc-def.apps.googleusercontent.com",
            "123-.apps.googleusercontent.com",
            "123-abc.apps.googleusercontent.com.evil.example",
            "123-abc/x.apps.googleusercontent.com",
        ] {
            assert!(!is_google_client_id(bad), "{bad:?}");
        }
        assert!(Service::Slack.built() && Service::Google.built());
        assert!(!Service::Hubspot.built());
        assert!(Service::Slack.many() && !Service::Google.many());
        assert!(!Part::Search.has_full_access());
        assert!(Part::Channels.has_full_access());
        assert_eq!(
            Service::Google.parts(),
            [Part::Gmail, Part::Calendar, Part::Drive]
        );
    }

    #[test]
    fn a_line_for_the_agent_wins_over_its_roles() {
        let mut c = connected();
        c.access = vec![
            Access {
                who: Who::Role {
                    id: "writer".into(),
                },
                level: AccessLevel::ReadWrite,
            },
            Access {
                who: Who::Agent { id: "pos-1".into() },
                level: AccessLevel::ReadOnly,
            },
        ];
        assert_eq!(
            c.line_for(Some("pos-1"), "writer").map(|a| a.level),
            Some(AccessLevel::ReadOnly)
        );
        assert_eq!(
            c.line_for(Some("pos-2"), "writer").map(|a| a.level),
            Some(AccessLevel::ReadWrite)
        );
        assert_eq!(c.line_for(None, "dev"), None);
    }

    #[test]
    fn connection_ids_and_guids() {
        assert_eq!(service_of("microsoft365"), Some(Service::Microsoft365));
        assert_eq!(service_of("slack-2"), Some(Service::Slack));
        assert_eq!(service_of("microsoft365-2"), None);
        assert_eq!(service_of("slack-x"), None);
        for not_one in [
            "slack-",
            "slack-0",
            "slack-1",
            "slack-02",
            "slack-1000",
            "slack-+2",
        ] {
            assert_eq!(service_of(not_one), None, "{not_one}");
        }
        assert_eq!(service_of("slack-999"), Some(Service::Slack));
        assert!(is_guid("12345678-abcd-4ef0-9abc-0123456789ab"));
        assert!(!is_guid("12345678-abcd-4ef0-9abc-0123456789a"));
        assert!(!is_guid("not-a-guid"));
    }

    #[test]
    fn the_owners_actions_are_checked() {
        let ok = ConnectionRequest {
            connection_id: "microsoft365",
            action: ConnectionAction::Connect,
            signing_in: false,
            has_app: true,
            parts_on: true,
        };
        assert_eq!(decide(&ok), Ok(()));
        let waiting = ConnectionRequest {
            signing_in: true,
            ..ok.clone()
        };
        assert!(decide(&waiting).unwrap_err().contains("already waiting"));
        let no_app = ConnectionRequest {
            has_app: false,
            ..ok.clone()
        };
        assert!(decide(&no_app).unwrap_err().contains("no app ID"));
        let no_parts = ConnectionRequest {
            parts_on: false,
            ..ok.clone()
        };
        assert!(decide(&no_parts).unwrap_err().contains("at least one part"));
        let later = ConnectionRequest {
            connection_id: "hubspot",
            ..ok.clone()
        };
        assert!(decide(&later).unwrap_err().contains("later update"));
        let unknown = ConnectionRequest {
            connection_id: "myspace",
            ..ok.clone()
        };
        assert!(decide(&unknown).unwrap_err().contains("no connection"));
        // Disconnecting is never refused for a known connection, built or not.
        for id in ["microsoft365", "slack", "hubspot"] {
            let off = ConnectionRequest {
                connection_id: id,
                action: ConnectionAction::Disconnect,
                has_app: false,
                ..ok.clone()
            };
            assert_eq!(decide(&off), Ok(()));
        }
        let cancel = ConnectionRequest {
            action: ConnectionAction::Cancel,
            ..ok.clone()
        };
        assert!(decide(&cancel).unwrap_err().contains("No sign-in"));
    }
}
