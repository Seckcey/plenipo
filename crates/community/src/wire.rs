//! Every request and answer in the contract (`contracts/community/v1`), named as its schema
//! names them. The tests in `tests/contract.rs` read every example in the contract's folder
//! into these types.
//!
//! - **Answers** from the service keep fields Plenipo does not know out of the way (contract
//!   §1: "Plenipo ignores fields it does not know in an answer").
//! - **Requests** Plenipo sends, and **what travels sealed** between PCs, refuse any field the
//!   contract does not name.

use serde::{Deserialize, Serialize};

/// The kinds of sealed item (contract §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Message,
    Reaction,
    LinkNote,
    Objective,
    ObjectiveState,
    Answer,
    CollabNote,
}

impl ItemKind {
    /// The contract's name for this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Reaction => "reaction",
            Self::LinkNote => "link_note",
            Self::Objective => "objective",
            Self::ObjectiveState => "objective_state",
            Self::Answer => "answer",
            Self::CollabNote => "collab_note",
        }
    }

    /// The largest sealed copy of this kind, before base64url (contract §15).
    pub fn most_sealed_bytes(self) -> usize {
        match self {
            Self::Objective | Self::Answer => 128 * 1024,
            _ => 32 * 1024,
        }
    }
}

/// One PC's public keys (`DevicePublic`, contract §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePublic {
    pub device_id: String,
    /// Ed25519, 32 bytes, base64url.
    pub signing_key: String,
    /// X25519, 32 bytes, base64url.
    pub sealing_key: String,
}

/// What is inside a seal (`SealedContent`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedContent {
    /// The item's [`ItemPayload`] as JSON, base64url, exactly as signed.
    pub payload: String,
    /// The sending PC's signature over the payload's text.
    pub sig: String,
    /// The report key, made for this item.
    pub fk: String,
}

/// The item itself (`ItemPayload`, contract §7). `B` is its body: a typed body when Plenipo
/// writes one, and plain JSON when Plenipo has just opened one and has not yet checked its body
/// for its kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemPayload<B = serde_json::Value> {
    pub v: u8,
    pub item_id: String,
    pub kind: ItemKind,
    pub from: String,
    pub from_device: String,
    pub to: String,
    /// The link or collaboration the item belongs to, or none.
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub sent_at: i64,
    pub body: B,
}

/// A GIF in a message: the library and the GIF's ID there, never a picture or an address
/// (ADR-164 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gif {
    pub library: String,
    pub id: String,
}

/// A sticker in a message: a set built into Plenipo, and a name in it. Plenipo has none yet
/// (ADR-172 §1); a message holding one shows "A sticker".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sticker {
    pub set: String,
    pub name: String,
}

/// A message's body (`MessageBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageBody {
    /// Up to 4,000 characters, or none.
    pub text: Option<String>,
    pub gif: Option<Gif>,
    pub sticker: Option<Sticker>,
    /// The message this one answers, or none.
    pub reply_to: Option<String>,
}

/// A reaction's body (`ReactionBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReactionBody {
    /// The item reacted to.
    pub item: String,
    /// One emoji, or none to take the reaction back.
    pub emoji: Option<String>,
}

/// 8 West's stamp's payload (`StampPayload`, contract §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StampPayload {
    pub v: u8,
    pub item_id: String,
    pub from: String,
    pub to: String,
    pub kind: ItemKind,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub tag: String,
    pub at: i64,
    /// The name of the stamping key.
    pub signer: String,
}

/// One sealed copy, for one PC (`ItemCopy`, contract §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemCopy {
    pub device_id: String,
    pub sealed: String,
}

// Small lists of words, used inside the bodies below.

/// A badge a member can hold (`Badge`, contract §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Badge {
    FoundingMember,
    Helper,
    Connector,
    GoodNeighbor,
    Trusted,
    TopHelper,
    /// A word a newer account service uses that this copy of Plenipo does not know yet. Read,
    /// so the rest of the answer still is, and then shown as nothing or left alone.
    #[serde(other)]
    Unknown,
}

/// The kinds of business a member can show (`BusinessKind`, contract §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusinessKind {
    Accounting,
    Agriculture,
    Automotive,
    Construction,
    Consulting,
    Design,
    Education,
    Energy,
    Engineering,
    Finance,
    FoodAndDrink,
    Government,
    Healthcare,
    Hospitality,
    Insurance,
    ItServices,
    Legal,
    Logistics,
    Manufacturing,
    Marketing,
    Media,
    Nonprofit,
    RealEstate,
    Retail,
    Security,
    Software,
    Telecom,
    Trades,
    Travel,
    Other,
}

/// A profile part 8 West hid (`HiddenPart`, contract §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HiddenPart {
    Picture,
    DisplayName,
    Message,
    Company,
    BusinessLine,
    /// A word a newer account service uses that this copy of Plenipo does not know yet. Read,
    /// so the rest of the answer still is, and then shown as nothing or left alone.
    #[serde(other)]
    Unknown,
}

/// The status a member sets (`Profile.status`, contract §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    Available,
    Busy,
    Away,
}

/// The status on a card: the member's own, or `offline` (`Card.status`, contract §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    Available,
    Busy,
    Away,
    Offline,
    /// A word a newer account service uses that this copy of Plenipo does not know yet. Read,
    /// so the rest of the answer still is, and then shown as nothing or left alone.
    #[serde(other)]
    Unknown,
}

/// A member's mood (`Profile.mood`, contract §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Great,
    Good,
    Okay,
    Tired,
    Stressed,
    Focused,
    Celebrating,
}

/// Adult (18 or older) or teen (13 to 17) (`Member.age_group`, contract §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgeGroup {
    Adult,
    Teen,
}

/// A member's standing (`Member.standing`, contract §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    Ok,
    Paused,
    Ended,
}

/// A standing in a notice, which may also be a warning (`Notice.standing`, contract §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeStanding {
    Ok,
    Warned,
    Paused,
    Ended,
}

/// Why points were given or taken (`PointsReason`, contract §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointsReason {
    ContactAccepted,
    Thanks,
    InviteJoined,
    #[serde(rename = "link_7_days")]
    Link7Days,
    #[serde(rename = "collab_7_days")]
    Collab7Days,
    GettingStarted,
    GoodMonth,
    InviteBoughtPro,
    TakenBack,
    #[serde(rename = "removed_by_8west")]
    RemovedBy8West,
    /// A word a newer account service uses that this copy of Plenipo does not know yet. Read,
    /// so the rest of the answer still is, and then shown as nothing or left alone.
    #[serde(other)]
    Unknown,
}

// Errors, the empty answer, and what is open (contract §1).

/// What the service answers when something goes wrong (`Error`, contract §1). `error` stays a
/// plain string, so a new code never stops an answer from being read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    pub error: String,
    /// One plain sentence Plenipo may show.
    pub message: String,
    /// `proof_failed` only: the index of the item that failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u32>,
}

/// An answer with nothing in it: `{}` (`Empty`, contract §13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Empty {}

/// Which parts of Community are open yet (`Open`, contract §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Open {
    /// Whether linked organizations (contract §9) are open.
    pub links: bool,
    /// Whether collaborators (contract §10) are open.
    pub collaborators: bool,
}

// Signing in from a PC (contract §2).

/// Starts a sign-in (`SignInStart`, contract §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignInStart {
    /// The PC's name as the person will see it, 1 to 60 characters.
    pub device_name: String,
    /// Plenipo's version, like `1.20.0`.
    pub app_version: String,
    /// Ed25519, 32 bytes, base64url.
    pub signing_key: String,
    /// X25519, 32 bytes, base64url.
    pub sealing_key: String,
}

/// The code the person types on the account site (`SignInStarted`, contract §2).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignInStarted {
    /// Secret. Stays on this PC.
    pub device_code: String,
    /// Like `4KQ-7TD`.
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds until the code runs out (600).
    pub expires_in: u32,
    /// Seconds between asks (5).
    pub interval: u32,
}

/// Asks if the person has allowed the sign-in yet (`SignInToken`, contract §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignInToken {
    pub device_code: String,
    /// This PC's signature over the device code, base64url.
    pub proof: String,
}

/// The end of a sign-in (`SignedIn`, contract §2). `pass` is a secret: never log it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIn {
    pub pass: String,
    pub device_id: String,
    pub me: Me,
}

// You, your membership, and your profile (contract §3).

/// One PC signed in to the account (`Device`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub device_id: String,
    pub name: String,
    pub added_at: i64,
    pub last_seen_at: i64,
}

/// The account's name (`Me.account`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
}

/// What a member shows of themselves (`Profile`, contract §3). Sent with `PUT .../me/profile`
/// and read back inside [`Me`]; `null` (or `[]`) is a part the member hides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub display_name: Option<String>,
    pub status: Option<ProfileStatus>,
    pub mood: Option<Mood>,
    pub message: Option<String>,
    pub company: Option<String>,
    /// Up to 3, no repeats.
    pub business_kinds: Vec<BusinessKind>,
    pub business_line: Option<String>,
    /// A country (`US`) or a US state (`US-CA`).
    pub region: Option<String>,
}

/// The account's membership in Community (`Member`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub member_id: String,
    pub name: String,
    pub joined_at: i64,
    pub age_group: AgeGroup,
    pub standing: Standing,
    pub paused_until: Option<i64>,
    pub terms_accepted: String,
    pub can_start: bool,
    pub appear_offline: bool,
    pub profile: Profile,
    pub has_picture: bool,
    pub picture_version: Option<String>,
    pub name_change_at: Option<i64>,
    /// Parts 8 West hid. They stay hidden whatever the PC sends.
    pub hidden_parts: Vec<HiddenPart>,
}

/// You, your PCs, and your membership (`Me`, contract §3). `member` is `null` until the account
/// joins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub account: Account,
    pub device_id: String,
    pub devices: Vec<Device>,
    /// The current Community terms version.
    pub terms: String,
    pub member: Option<Member>,
}

/// Joins Community (`Join`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Join {
    pub name: String,
    pub birth_month: u8,
    pub birth_year: u16,
    /// The terms version the person accepted on screen.
    pub terms: String,
}

/// Changes the Community name (`NameChange`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameChange {
    pub name: String,
}

/// Accepts the Community terms (`TermsAccept`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TermsAccept {
    pub terms: String,
}

/// A new picture (`PictureUpload`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureUpload {
    /// The PNG, base64url.
    pub png: String,
}

/// Appear offline, or not (`Presence`, contract §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
    pub appear_offline: bool,
}

// Finding people (contract §4).

/// What one member may see of another (`Card`, contract §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub member_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub status: Option<CardStatus>,
    pub mood: Option<Mood>,
    pub message: Option<String>,
    pub company: Option<String>,
    pub business_kinds: Vec<BusinessKind>,
    pub business_line: Option<String>,
    pub region: Option<String>,
    pub has_picture: bool,
    pub picture_version: Option<String>,
    pub badges: Vec<Badge>,
    /// All time.
    pub points: i64,
    /// How many different people thanked this member.
    pub thanked_by: u32,
}

/// A page of cards (`CardPage`, contract §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardPage {
    pub cards: Vec<Card>,
    /// Where the next page starts, or none.
    pub next: Option<String>,
}

/// A member who can only be sent a message request (`RequestOnly`, contract §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestOnly {
    pub member_id: String,
    pub name: String,
    /// Always `true`.
    pub request_only: bool,
}

/// The PCs of one member (`Devices`, contract §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Devices {
    pub member_id: String,
    pub devices: Vec<DevicePublic>,
}

// Conversations (contract §5).

/// Where a conversation stands (`Contact.state`, contract §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactState {
    RequestedByMe,
    RequestedByThem,
    Accepted,
    LeftByMe,
    LeftByThem,
    /// A word a newer account service uses that this copy of Plenipo does not know yet. Read,
    /// so the rest of the answer still is, and then shown as nothing or left alone.
    #[serde(other)]
    Unknown,
}

/// One person this member talks with, or has asked or been asked (`Contact`, contract §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub member_id: String,
    pub name: String,
    /// Only while the two talk; otherwise none.
    pub display_name: Option<String>,
    pub state: ContactState,
    pub since: i64,
}

/// All of this member's contacts (`Contacts`, contract §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contacts {
    pub contacts: Vec<Contact>,
}

// Sealed items: sending, picking up, and the stamp (contract §6).

/// Sends one sealed item (`ItemSend`, contract §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSend {
    pub item_id: String,
    pub to: String,
    pub kind: ItemKind,
    /// The link or collaboration the item belongs to, or none.
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    /// The report proof's commitment, base64url.
    pub tag: String,
    pub copies: Vec<ItemCopy>,
}

/// What the service answers when it takes an item (`ItemSent`, contract §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSent {
    pub item_id: String,
    pub accepted_at: i64,
    /// 8 West's stamp: `<payload>.<signature>`. The sending PC keeps it.
    pub stamp: String,
    /// Whether the item was a message request.
    pub request: bool,
}

/// What a notice is about (`Notice.type`, contract §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeType {
    ContactAccepted,
    LinkRequested,
    LinkAccepted,
    LinkPaused,
    LinkEnded,
    CollabInvited,
    CollabAccepted,
    CollabEnded,
    StandingChanged,
    ReportClosed,
    MyDevicesChanged,
    ProfileHidden,
    /// A word a newer account service uses that this copy of Plenipo does not know yet. Read,
    /// so the rest of the answer still is, and then shown as nothing or left alone.
    #[serde(other)]
    Unknown,
}

/// How 8 West finished a report (`Notice.outcome`, contract §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportOutcome {
    Action,
    NoAction,
}

/// Something the service itself tells this PC (`Notice`, contract §6). Its `type` decides which
/// of the other fields are there; the rest are none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    #[serde(rename = "type")]
    pub kind: NoticeType,
    #[serde(default)]
    pub member_id: Option<String>,
    #[serde(default)]
    pub link_id: Option<String>,
    #[serde(default)]
    pub collab_id: Option<String>,
    #[serde(default)]
    pub paused: Option<bool>,
    #[serde(default)]
    pub standing: Option<NoticeStanding>,
    #[serde(default)]
    pub paused_until: Option<i64>,
    #[serde(default)]
    pub report_id: Option<String>,
    #[serde(default)]
    pub outcome: Option<ReportOutcome>,
    /// Every part hidden now.
    #[serde(default)]
    pub parts: Option<Vec<HiddenPart>>,
}

/// What an inbox item is: a sealed item of some kind, or a notice the service made
/// (`InboxItem.kind`, contract §6). It is one word in JSON, so it is read from one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum InboxKind {
    Item(ItemKind),
    Notice,
}

impl InboxKind {
    /// The contract's name for this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Item(kind) => kind.as_str(),
            Self::Notice => "notice",
        }
    }
}

impl From<InboxKind> for String {
    fn from(kind: InboxKind) -> Self {
        kind.as_str().to_owned()
    }
}

impl TryFrom<String> for InboxKind {
    type Error = String;

    fn try_from(word: String) -> Result<Self, String> {
        if word == "notice" {
            return Ok(Self::Notice);
        }
        serde_json::from_value(serde_json::Value::String(word.clone()))
            .map(Self::Item)
            .map_err(|_| format!("`{word}` is not a kind of inbox item"))
    }
}

/// One thing waiting for this PC (`InboxItem`, contract §6). For a notice, `from`, `sealed`,
/// `tag`, and `stamp` are none and `notice` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub item_id: String,
    pub kind: InboxKind,
    pub from: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    /// This PC's sealed copy, base64url.
    pub sealed: Option<String>,
    pub tag: Option<String>,
    pub stamp: Option<String>,
    pub accepted_at: i64,
    /// Whether it is a message request.
    pub request: bool,
    pub notice: Option<Notice>,
}

/// Up to 50 things waiting for this PC, oldest first (`Inbox`, contract §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inbox {
    pub items: Vec<InboxItem>,
    /// Whether more are waiting.
    pub more: bool,
}

/// Says which items this PC is done with (`Ack`, contract §6). Up to 100.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ack {
    pub item_ids: Vec<String>,
}

// What is inside a seal (contract §7). These travel sealed between PCs, so they refuse any
// field the contract does not name.

/// A link note's body (`LinkNoteBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkNoteBody {
    /// Up to 80 characters.
    pub org_name: String,
    /// Up to 500 characters, or none.
    pub note: Option<String>,
}

/// An objective's body (`ObjectiveBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveBody {
    pub org_name: String,
    /// Up to 20,000 characters.
    pub text: String,
}

/// What became of an objective (`ObjectiveStateBody.state`, contract §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveState {
    Approved,
    Refused,
    Stopped,
    Finished,
}

/// An objective's new state (`ObjectiveStateBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveStateBody {
    /// The objective's item ID.
    pub objective: String,
    pub state: ObjectiveState,
    /// Up to 200 characters, or none.
    pub why: Option<String>,
}

/// An answer to an objective (`AnswerBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerBody {
    /// The objective's item ID.
    pub objective: String,
    /// Up to 20,000 characters.
    pub text: String,
}

/// What a collaborator may do (`CollabNoteBody.role`, contract §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollabRole {
    Viewer,
    Approver,
    Manager,
}

/// A kind of part of an organization (`CollabNoteBody.part`, contract §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Department,
    Project,
}

/// One department or project a collaborator may reach (contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartPick {
    pub kind: PartKind,
    /// Up to 80 characters.
    pub name: String,
}

/// The word `whole` (the first half of `CollabNoteBody.part`, contract §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WholePart {
    Whole,
}

/// How much of the organization a collaborator reaches (`CollabNoteBody.part`, contract §7):
/// `whole`, or a list of up to 50 departments and projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CollabPart {
    Whole(WholePart),
    Only(Vec<PartPick>),
}

/// The kinds of approval only the owner answers (`CollabNoteBody.owner_only`, contract §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerOnly {
    ProductionChanges,
    Dns,
    Credentials,
    Databases,
    CloudDeletions,
    Payments,
    MessagesAndPublishing,
    Administrator,
    OutsideProjectFolder,
    ProductionServerCommands,
    LinkedObjectives,
}

/// A collaboration invitation's details (`CollabNoteBody`, contract §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollabNoteBody {
    pub org_name: String,
    pub role: CollabRole,
    pub part: CollabPart,
    pub owner_only: Vec<OwnerOnly>,
}

// Blocks (contract §8).

/// One member this member blocked (`Block`, contract §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub member_id: String,
    pub name: String,
    pub blocked_at: i64,
}

/// All of this member's blocks (`Blocks`, contract §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocks {
    pub blocks: Vec<Block>,
}

// Linked organizations (contract §9).

/// Asks to link (`LinkAsk`, contract §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkAsk {
    pub to: String,
    /// The `co_` this PC made for the link.
    pub org_ref: String,
}

/// Accepts a link (`LinkAccept`, contract §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkAccept {
    pub org_ref: String,
}

/// Pauses or unpauses objectives on one side of a link (`LinkPause`, contract §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkPause {
    pub paused: bool,
}

/// Who asked for a link (`Link.asked_by`, contract §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskedBy {
    Me,
    Them,
}

/// Where a link stands (`Link.state`, contract §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    Requested,
    Active,
    Ended,
}

/// A link between two members' organizations (`Link`, contract §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub link_id: String,
    /// The other member.
    pub other: String,
    pub asked_by: AskedBy,
    pub my_org_ref: Option<String>,
    /// None until the link is accepted.
    pub their_org_ref: Option<String>,
    pub state: LinkState,
    pub paused_by_me: bool,
    pub paused_by_them: bool,
    pub created_at: i64,
    pub accepted_at: Option<i64>,
    pub ended_at: Option<i64>,
}

/// All of this member's links (`Links`, contract §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Links {
    pub links: Vec<Link>,
}

// Collaborators (contract §10).

/// Invites a member to help with an organization (`CollabInvite`, contract §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollabInvite {
    pub to: String,
    /// The `co_` this PC made for the collaboration.
    pub org_ref: String,
}

/// Where a collaboration stands (`Collaboration.state`, contract §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollabState {
    Invited,
    Active,
    Ended,
}

/// A person helping with an organization (`Collaboration`, contract §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collaboration {
    pub collab_id: String,
    pub owner: String,
    pub collaborator: String,
    pub org_ref: String,
    pub state: CollabState,
    pub created_at: i64,
    pub accepted_at: Option<i64>,
    pub ended_at: Option<i64>,
    /// For an invitation: when it lapses.
    pub expires_at: Option<i64>,
}

/// All of this member's collaborations (`Collaborations`, contract §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collaborations {
    pub collaborations: Vec<Collaboration>,
}

// Reports (contract §11).

/// Why a member is reported (`Report.reason`, contract §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportReason {
    Spam,
    Harassment,
    Scam,
    Hate,
    Sexual,
    #[serde(rename = "under_13")]
    Under13,
    YoungPersonRisk,
    Impersonation,
    Cheating,
    Other,
}

/// What a report is about (`Report.what`, contract §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportWhat {
    Person,
    Profile,
    Items,
}

/// One item sent as proof in a report (`ReportItem`, contract §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportItem {
    /// 8 West's stamp, as it came.
    pub stamp: String,
    /// The item's payload, base64url, as it came.
    pub payload: String,
    /// The report key, base64url.
    pub fk: String,
}

/// Reports a member (`Report`, contract §11). `items` holds 1 to 20 proofs for `items`, and
/// is empty for the other two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub about: String,
    pub reason: ReportReason,
    /// Up to 1,000 characters, or none.
    pub note: Option<String>,
    pub what: ReportWhat,
    pub items: Vec<ReportItem>,
}

/// The answer to a report (`ReportMade`, contract §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportMade {
    pub report_id: String,
}

// Thanks, points, badges, and the leaderboard (contract §12).

/// What a thanks is for (`Thanks.for`, contract §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThanksFor {
    LinkAnswer,
    Collaborator,
}

/// Thanks a member (`Thanks`, contract §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thanks {
    pub to: String,
    #[serde(rename = "for")]
    pub for_what: ThanksFor,
    /// The link or collaboration it was for.
    #[serde(rename = "ref")]
    pub reference: String,
}

/// One change to a member's points (`Points.recent`, contract §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointsChange {
    pub points: i64,
    pub reason: PointsReason,
    pub at: i64,
}

/// Free months earned (`Points.free_months`, contract §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreeMonths {
    pub this_year: u32,
    pub max: u32,
}

/// This member's points (`Points`, contract §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Points {
    pub total: i64,
    pub week: i64,
    pub week_started_at: i64,
    pub place_week: Option<u32>,
    pub place_all: Option<u32>,
    pub badges: Vec<Badge>,
    pub thanked_by: u32,
    /// The last 20 changes.
    pub recent: Vec<PointsChange>,
    /// None for a member under 18.
    pub free_months: Option<FreeMonths>,
}

/// The week, or all time (`Leaderboard.period`, contract §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardPeriod {
    Week,
    All,
}

/// One place on the leaderboard (`Leaderboard.top`, contract §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderboardRow {
    pub place: u32,
    pub member_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub has_picture: bool,
    pub picture_version: Option<String>,
    pub badges: Vec<Badge>,
    pub points: i64,
}

/// The asking member's own place (`Leaderboard.me`, contract §12). `place` is none when the
/// member is left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderboardMe {
    pub place: Option<u32>,
    pub points: i64,
}

/// The leaderboard (`Leaderboard`, contract §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaderboard {
    pub period: LeaderboardPeriod,
    pub since: Option<i64>,
    /// Up to 50.
    pub top: Vec<LeaderboardRow>,
    pub me: Option<LeaderboardMe>,
}

// Inviting by email (contract §13), and GIFs (contract §14).

/// Invites someone by email (`InviteEmail`, contract §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteEmail {
    pub email: String,
}

/// One GIF found by a search (`Gifs.results`, contract §14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GifResult {
    pub id: String,
    pub width: u32,
    pub height: u32,
    /// An `https` address on the library's picture host.
    pub preview: String,
}

/// The answer to a GIF search (`Gifs`, contract §14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gifs {
    pub library: String,
    /// Words Plenipo must show.
    pub attribution: String,
    pub results: Vec<GifResult>,
    pub next_offset: Option<u32>,
}

// The sign-in's secrets are never shown: not the device code, and never the pass.

impl std::fmt::Debug for SignInStarted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignInStarted")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for SignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedIn")
            .field("device_id", &self.device_id)
            .field("me", &self.me)
            .finish_non_exhaustive()
    }
}
