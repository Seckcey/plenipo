//! Private messages (ADR-164): one to one, sealed on this PC for each of the other person's PCs
//! and each of your other PCs, picked up and checked here, and kept in the PC's shared record
//! (the first organization's Ledger, ADR-164 §10). 8 West sees only the envelope.
//!
//! - **Requests** (ADR-164 §7): a first message from someone you don't talk with waits in
//!   Requests, with **Accept**, **Block**, and **Report**. Until you accept, they can send nothing
//!   more.
//! - **The safety code** (ADR-164 §2) is worked out from both people's PCs each time they are
//!   fetched. When it changes, the conversation says "**Pat's computers changed**"; it never stops
//!   a message.
//! - **Delete for me** removes a message from this PC only (ADR-172).
//! - The Ledger's events say that a conversation started, was accepted, or was left, and the
//!   person's Community name; never a word of a message. Logs never hold a message, a key, a tag,
//!   or a stamp (ADR-164 §12).
//!
//! What other people write is shown as text, never as a web page; that is the screen's job
//! (`safeText.ts`). This module checks only what the contract checks: who it is from, that
//! nothing changed, and that it is one of the kinds 24C shows.

use std::collections::BTreeMap;

use plenipo_ledger::community::{
    CommunityItem, CommunityPerson, ItemKind as KeptKind, ItemProof, ItemState, PersonState,
};
use plenipo_ledger::Ledger;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::client::{self, ErrorCode, Failure, Transport};
use crate::ids::{self, IdKind};
use crate::item::{self, Envelope, ThisPc};
use crate::service::{words, Community, Refused};
use crate::wire::{self, ContactState, DevicePublic, ItemKind, ItemPayload, NoticeType};

/// The longest message (contract §7). Counted in characters, as the contract counts them.
pub const MOST_TEXT_CHARS: usize = 4000;
/// The reactions Plenipo offers (ADR-164 §4). One emoji arriving from another copy of Plenipo is
/// shown as it is, as text.
pub const REACTIONS: [&str; 5] = ["👍", "❤️", "😂", "😮", "🙏"];
/// The longest reaction Plenipo keeps: one emoji, with its joiners and skin tone.
const MOST_EMOJI_CHARS: usize = 16;
/// Picking up asks for at most this many seconds of waiting (contract §6).
pub const PICK_UP_WAIT_SECS: u32 = 25;
/// The most item IDs in one **Done** (contract §6).
const MOST_ACK: usize = 100;
/// Sealing again after 8 West says the PCs changed: at most this many times.
const SEAL_TRIES: usize = 3;

/// One conversation in the list, or one waiting in **Requests**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationSummary {
    pub member_id: String,
    /// Their Community name, without the `@`.
    pub name: String,
    pub display_name: Option<String>,
    /// `none`, `requestedByMe`, `requestedByThem`, `accepted`, `leftByMe`, or `leftByThem`.
    pub state: String,
    /// Messages from them you haven't seen.
    pub unseen: u32,
    /// When anything last happened (Unix seconds).
    #[ts(type = "number")]
    pub last_at: i64,
    /// Their PCs, or yours, changed since you last looked: "**Pat's computers changed**".
    pub computers_changed: bool,
}

/// One reaction on a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReactionView {
    pub emoji: String,
    /// Yours, not theirs.
    pub mine: bool,
}

/// One message, as the screen shows it. The words are shown as text only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MessageView {
    pub item_id: String,
    /// Yours.
    pub outgoing: bool,
    pub text: Option<String>,
    /// It holds a GIF or a sticker, which this version doesn't show (ADR-172).
    pub has_gif: bool,
    pub has_sticker: bool,
    /// The message this one answers, if it is still on this PC.
    pub reply_to: Option<String>,
    #[ts(type = "number")]
    pub sent_at: i64,
    #[ts(type = "number | null")]
    pub accepted_at: Option<i64>,
    /// `waiting`, `delivered`, `notDelivered`, or `received`.
    pub state: String,
    /// A first message: a request.
    pub request: bool,
    pub reactions: Vec<ReactionView>,
    /// It can be reported (theirs, with its proof).
    pub reportable: bool,
}

/// One conversation, as the screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversationView {
    pub person: ConversationSummary,
    /// The safety code, as `5373 9207 7552`, once both people's PCs were fetched.
    pub safety_code: Option<String>,
    /// Oldest first.
    pub messages: Vec<MessageView>,
}

/// What one pick-up brought.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickedUp {
    /// New messages and reactions kept.
    pub kept: usize,
    /// Items dropped because a check failed.
    pub dropped: usize,
    /// Notices from 8 West.
    pub notices: usize,
    /// More are waiting: pick up again at once.
    pub more: bool,
    /// Something about you changed (your standing, a hidden part): ask who you are again.
    pub me_changed: bool,
}

fn refused(failure: &Failure) -> Refused {
    Refused(words(failure))
}

fn kept(e: plenipo_ledger::LedgerError) -> Refused {
    log::warn!("Community could not keep a message on this PC: {e}");
    Refused("Plenipo couldn't save this on this computer.".into())
}

/// The safety code without its spaces, as the Ledger keeps it.
fn digits(code: &str) -> String {
    code.chars().filter(char::is_ascii_digit).collect()
}

/// `5373 9207 7552` again, from 12 digits.
fn grouped(code: &str) -> String {
    if code.len() == 12 {
        format!("{} {} {}", &code[0..4], &code[4..8], &code[8..12])
    } else {
        code.to_owned()
    }
}

fn state_word(state: PersonState) -> String {
    state.as_str().to_owned()
}

fn person_state(state: ContactState) -> Option<PersonState> {
    Some(match state {
        ContactState::RequestedByMe => PersonState::RequestedByMe,
        ContactState::RequestedByThem => PersonState::RequestedByThem,
        ContactState::Accepted => PersonState::Accepted,
        ContactState::LeftByMe => PersonState::LeftByMe,
        ContactState::LeftByThem => PersonState::LeftByThem,
        ContactState::Unknown => return None,
    })
}

/// A message's words, checked: up to 4,000 characters. Every character is allowed, in any
/// language and any direction (ADR-164 §4); the screen shows hidden ones as visible marks.
fn message_text(text: &str) -> Result<String, Refused> {
    if text.trim().is_empty() {
        return Err(Refused("Write a message first.".into()));
    }
    if text.chars().count() > MOST_TEXT_CHARS {
        return Err(Refused(format!(
            "A message can be at most {MOST_TEXT_CHARS} characters."
        )));
    }
    Ok(text.to_owned())
}

/// A message's body that arrived, checked for what 24C keeps: words, or a GIF or a sticker this
/// version shows only as "A GIF" or "A sticker".
fn arrived_message(body: &Value) -> Option<wire::MessageBody> {
    let body: wire::MessageBody = serde_json::from_value(body.clone()).ok()?;
    let has_text = body.text.as_deref().is_some_and(|t| !t.is_empty());
    let text_fits = body
        .text
        .as_deref()
        .is_none_or(|t| t.chars().count() <= MOST_TEXT_CHARS);
    let reply_fits = body
        .reply_to
        .as_deref()
        .is_none_or(|r| ids::is_id(IdKind::Item, r));
    let something = has_text || body.gif.is_some() || body.sticker.is_some();
    let one_picture = !(body.gif.is_some() && body.sticker.is_some());
    (text_fits && reply_fits && something && one_picture).then_some(body)
}

/// A reaction that arrived, checked: to an item, with one short emoji or none.
fn arrived_reaction(body: &Value) -> Option<wire::ReactionBody> {
    let body: wire::ReactionBody = serde_json::from_value(body.clone()).ok()?;
    let emoji_fits = body.emoji.as_deref().is_none_or(|e| {
        let n = e.chars().count();
        (1..=MOST_EMOJI_CHARS).contains(&n) && !e.chars().any(char::is_whitespace)
    });
    (ids::is_id(IdKind::Item, &body.item) && emoji_fits).then_some(body)
}

/// Change only where the conversation stands, on the person as kept now (their safety code may
/// have changed since they were read).
fn set_state(ledger: &Ledger, member_id: &str, state: PersonState) -> Result<(), Refused> {
    if let Some(mut person) = ledger.community_person(member_id).map_err(kept)? {
        person.state = state;
        ledger.community_put_person(&person).map_err(kept)?;
    }
    Ok(())
}

/// Why an item that arrived was not kept now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotKept {
    /// A check failed: dropped for good, and the reason recorded.
    Dropped(&'static str),
    /// 8 West or this PC couldn't answer just now: left waiting, and picked up again next time.
    Later,
}

impl From<item::Dropped> for NotKept {
    fn from(why: item::Dropped) -> Self {
        Self::Dropped(why.as_str())
    }
}

/// Whether a failure is for now only: no answer, a busy service, or too many requests.
fn for_now(failure: &Failure) -> bool {
    match failure {
        Failure::Unreachable(_) | Failure::BadAnswer => true,
        Failure::Service(e) => {
            e.status >= 500 || matches!(e.code, ErrorCode::TooMany | ErrorCode::Unavailable)
        }
        Failure::NotSignedIn => true,
    }
}

/// Who this PC is now, to send or to open.
struct Me {
    member_id: String,
    pc: std::sync::Arc<crate::session::SignedInPc>,
}

impl<T: Transport> Community<T> {
    fn me_for_messages(&self) -> Result<Me, Refused> {
        let member_id = self
            .member_id()
            .ok_or_else(|| Refused("Join Community first.".into()))?;
        let pc = self
            .signed_in_pc()
            .ok_or_else(|| Refused("Sign in to Community first.".into()))?;
        Ok(Me { member_id, pc })
    }

    /// One member's PCs, as 8 West lists them now.
    async fn pcs_of(&self, member_id: &str) -> Result<Vec<DevicePublic>, Failure> {
        let request = client::devices_of(member_id).map_err(|_| Failure::BadAnswer)?;
        let answer = self.as_member(&request, 200).await?;
        let devices: wire::Devices = client::read(answer, 200)?;
        if devices.member_id != member_id {
            return Err(Failure::BadAnswer);
        }
        Ok(devices.devices)
    }

    /// Work out the safety code with both people's PCs, and keep it. The first code is the one
    /// you have seen; a different one later says their computers changed.
    fn keep_safety_code(
        &self,
        ledger: &Ledger,
        me: &Me,
        mine: &[DevicePublic],
        member_id: &str,
        theirs: &[DevicePublic],
    ) {
        let code = digits(&crate::safety::safety_code(
            (&me.member_id, mine),
            (member_id, theirs),
        ));
        if let Ok(Some(mut person)) = ledger.community_person(member_id) {
            if person.safety_code.as_deref() != Some(code.as_str()) {
                if person.safety_seen.is_none() {
                    person.safety_seen = Some(code.clone());
                }
                person.safety_code = Some(code);
                if let Err(e) = ledger.community_put_person(&person) {
                    log::warn!("Community could not keep a safety code: {e}");
                }
            }
        }
    }

    /// The person, from this PC, or from 8 West's list of your conversations.
    async fn person(
        &self,
        ledger: &Ledger,
        member_id: &str,
    ) -> Result<Option<CommunityPerson>, Refused> {
        if let Some(person) = ledger.community_person(member_id).map_err(kept)? {
            return Ok(Some(person));
        }
        self.sync_conversations(ledger).await?;
        ledger.community_person(member_id).map_err(kept)
    }

    /// Bring this PC's conversations up to date with 8 West's list: who asked whom, who accepted,
    /// who left (contract §5).
    pub async fn sync_conversations(&self, ledger: &Ledger) -> Result<(), Refused> {
        let answer = self
            .as_member(&client::contacts(), 200)
            .await
            .map_err(|f| refused(&f))?;
        let contacts: wire::Contacts = client::read(answer, 200).map_err(|f| refused(&f))?;
        for contact in contacts.contacts {
            let Some(state) = person_state(contact.state) else {
                continue;
            };
            if !ids::is_id(IdKind::Member, &contact.member_id)
                || !client::is_community_name(&contact.name)
            {
                continue;
            }
            let kept_person = ledger.community_person(&contact.member_id).map_err(kept)?;
            let person = match kept_person {
                Some(mut p) => {
                    p.name = contact.name;
                    p.state = state;
                    if contact.display_name.is_some() {
                        p.display_name = contact.display_name.map(|d| d.chars().take(60).collect());
                    }
                    p
                }
                None => CommunityPerson {
                    member_id: contact.member_id,
                    name: contact.name,
                    display_name: contact.display_name.map(|d| d.chars().take(60).collect()),
                    state,
                    safety_code: None,
                    safety_seen: None,
                    blocked: false,
                    updated_at: contact.since,
                },
            };
            ledger.community_put_person(&person).map_err(kept)?;
        }
        Ok(())
    }

    /// **Send** a message to a member: `name` is their Community name, as Find someone or a card
    /// showed it. A first message to someone you don't talk with is a request, which needs Pro
    /// (ADR-162 §5, ADR-164 §7).
    pub async fn send_message(
        &self,
        ledger: &Ledger,
        to: &str,
        name: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Result<MessageView, Refused> {
        let text = message_text(text)?;
        if reply_to.is_some_and(|r| !ids::is_id(IdKind::Item, r)) {
            return Err(Refused("That message isn't one you can answer.".into()));
        }
        let body = wire::MessageBody {
            text: Some(text),
            gif: None,
            sticker: None,
            reply_to: reply_to.map(str::to_owned),
        };
        self.send_item(ledger, to, name, ItemKind::Message, body)
            .await
    }

    /// React to a message with one emoji, or take your reaction back (`None`).
    pub async fn react(
        &self,
        ledger: &Ledger,
        item_id: &str,
        emoji: Option<&str>,
    ) -> Result<(), Refused> {
        if emoji.is_some_and(|e| !REACTIONS.contains(&e)) {
            return Err(Refused("Choose one of the reactions.".into()));
        }
        let message = ledger
            .community_item(item_id)
            .map_err(kept)?
            .filter(|i| i.kind == KeptKind::Message)
            .ok_or_else(|| Refused("That message isn't on this computer.".into()))?;
        let person = ledger
            .community_person(&message.member_id)
            .map_err(kept)?
            .ok_or_else(|| Refused("That message isn't on this computer.".into()))?;
        let body = wire::ReactionBody {
            item: item_id.to_owned(),
            emoji: emoji.map(str::to_owned),
        };
        self.send_item(
            ledger,
            &person.member_id,
            &person.name,
            ItemKind::Reaction,
            body,
        )
        .await
        .map(|_| ())
    }

    async fn send_item<B: Serialize>(
        &self,
        ledger: &Ledger,
        to: &str,
        name: &str,
        kind: ItemKind,
        body: B,
    ) -> Result<MessageView, Refused> {
        let me = self.me_for_messages()?;
        if !ids::is_id(IdKind::Member, to) || !client::is_community_name(name) {
            return Err(Refused("That isn't someone in Community.".into()));
        }
        if to == me.member_id {
            return Err(Refused("You can't send a message to yourself.".into()));
        }
        let person = match self.person(ledger, to).await? {
            Some(p) => p,
            None => CommunityPerson {
                member_id: to.to_owned(),
                name: name.to_owned(),
                display_name: None,
                state: PersonState::None,
                safety_code: None,
                safety_seen: None,
                blocked: false,
                updated_at: self.now(),
            },
        };
        if person.state == PersonState::RequestedByMe {
            return Err(Refused(format!(
                "Wait for @{} to accept your first message.",
                person.name
            )));
        }
        if person.state == PersonState::RequestedByThem && kind == ItemKind::Message {
            return Err(Refused(format!(
                "Accept @{}'s message first, or leave it.",
                person.name
            )));
        }
        let body = serde_json::to_value(&body).map_err(|_| Refused(words(&Failure::BadAnswer)))?;
        let payload = ItemPayload {
            v: 1,
            item_id: ids::new_id(IdKind::Item),
            kind,
            from: me.member_id.clone(),
            from_device: me.pc.device_id().to_owned(),
            to: to.to_owned(),
            reference: None,
            sent_at: self.now(),
            body: body.clone(),
        };
        ledger.community_put_person(&person).map_err(kept)?;
        let kept_kind = match kind {
            ItemKind::Reaction => KeptKind::Reaction,
            _ => KeptKind::Message,
        };
        ledger
            .community_add_item(&CommunityItem {
                item_id: payload.item_id.clone(),
                member_id: to.to_owned(),
                outgoing: true,
                kind: kept_kind,
                body,
                sent_at: payload.sent_at,
                accepted_at: None,
                request: false,
                state: ItemState::Waiting,
                proof: None,
                seen: true,
            })
            .map_err(kept)?;

        // Seal for their PCs and your other PCs as 8 West lists them now; if they changed on
        // the way, fetch them again and seal again.
        let mut last = Failure::BadAnswer;
        for _ in 0..SEAL_TRIES {
            let (theirs, mine) = match (self.pcs_of(to).await, self.pcs_of(&me.member_id).await) {
                (Ok(theirs), Ok(mine)) => (theirs, mine),
                (Err(f), _) | (_, Err(f)) => {
                    last = f;
                    break;
                }
            };
            self.keep_safety_code(ledger, &me, &mine, to, &theirs);
            let pcs: Vec<DevicePublic> = theirs
                .iter()
                .chain(mine.iter().filter(|d| d.device_id != me.pc.device_id()))
                .cloned()
                .collect();
            if theirs.is_empty() {
                last = Failure::Service(client::ServiceError {
                    status: 409,
                    code: ErrorCode::NotDelivered,
                    message: format!("@{} has no computer signed in to Community.", person.name),
                    retry_after: None,
                    item: None,
                });
                break;
            }
            let sealed = item::seal_item(me.pc.keys(), &payload, &pcs)
                .map_err(|e| Refused(e.to_string()))?;
            let send = wire::ItemSend {
                item_id: sealed.item_id.clone(),
                to: sealed.to.clone(),
                kind: sealed.kind,
                reference: None,
                tag: sealed.tag.clone(),
                copies: sealed.copies.clone(),
            };
            match self.as_member(&client::send_item(&send), 200).await {
                Ok(answer) => {
                    let sent: wire::ItemSent =
                        client::read(answer, 200).map_err(|f| refused(&f))?;
                    if sent.item_id != payload.item_id {
                        return Err(refused(&Failure::BadAnswer));
                    }
                    ledger
                        .community_set_item_state(
                            &sent.item_id,
                            ItemState::Delivered,
                            Some(sent.accepted_at),
                            Some(&sent.stamp),
                        )
                        .map_err(kept)?;
                    if sent.request && kind == ItemKind::Message {
                        set_state(ledger, to, PersonState::RequestedByMe)?;
                        self.record(
                            "community.conversation_started",
                            json!({ "name": person.name }),
                        );
                    } else if matches!(person.state, PersonState::LeftByMe | PersonState::None) {
                        // Writing again opens your own side (contract §5).
                        set_state(ledger, to, PersonState::Accepted)?;
                    }
                    let item = ledger
                        .community_item(&payload.item_id)
                        .map_err(kept)?
                        .ok_or_else(|| refused(&Failure::BadAnswer))?;
                    return Ok(view_of(&item, &[]));
                }
                Err(f) if f.code() == Some(&ErrorCode::DevicesChanged) => last = f,
                Err(f) => {
                    last = f;
                    break;
                }
            }
        }
        // Not taken. A refusal 8 West gave leaves nothing behind but "Not delivered" when the
        // other person left; anything else takes the message back off this PC.
        match last.code() {
            Some(ErrorCode::NotDelivered) => {
                ledger
                    .community_set_item_state(&payload.item_id, ItemState::NotDelivered, None, None)
                    .map_err(kept)?;
            }
            _ => {
                let _ = ledger.community_delete_item(&payload.item_id);
            }
        }
        if last.code() == Some(&ErrorCode::WaitingForAccept) {
            let _ = set_state(ledger, to, PersonState::RequestedByMe);
        }
        Err(refused(&last))
    }

    /// **Pick up** what waits for this PC (contract §6), up to `wait` seconds: check each item,
    /// keep what passes, and tell 8 West this PC is done with all of them (dropped ones too, so
    /// they never come back).
    pub async fn pick_up(&self, ledger: &Ledger, wait: u32) -> Result<PickedUp, Refused> {
        let me = self.me_for_messages()?;
        let answer = self
            .as_member(&client::pick_up(wait.min(PICK_UP_WAIT_SECS)), 200)
            .await
            .map_err(|f| refused(&f))?;
        let inbox: wire::Inbox = client::read(answer, 200).map_err(|f| refused(&f))?;
        let mut picked = PickedUp {
            more: inbox.more,
            ..PickedUp::default()
        };
        let mut done = Vec::with_capacity(inbox.items.len());
        let mut pcs: BTreeMap<String, Vec<DevicePublic>> = BTreeMap::new();
        for arrived in inbox.items {
            if !ids::is_id(IdKind::Item, &arrived.item_id) {
                continue;
            }
            done.push(arrived.item_id.clone());
            match arrived.kind {
                wire::InboxKind::Notice => {
                    picked.notices += 1;
                    if let Some(notice) = &arrived.notice {
                        self.heard(ledger, notice, &mut picked).await;
                    }
                }
                wire::InboxKind::Item(kind @ (ItemKind::Message | ItemKind::Reaction)) => {
                    match self.open(ledger, &me, kind, &arrived, &mut pcs).await {
                        Ok(true) => picked.kept += 1,
                        Ok(false) => {}
                        Err(NotKept::Dropped(why)) => {
                            picked.dropped += 1;
                            self.record("community.item_dropped", json!({ "why": why }));
                        }
                        Err(NotKept::Later) => {
                            // Not done with it: 8 West hands it out again next time.
                            done.pop();
                            picked.more = true;
                        }
                    }
                }
                // Links and collaborators (24D, 24E) are not open yet: 8 West sends none.
                wire::InboxKind::Item(_) => {}
            }
        }
        for chunk in done.chunks(MOST_ACK) {
            let body = wire::Ack {
                item_ids: chunk.to_vec(),
            };
            self.as_member(&client::ack(&body), 204)
                .await
                .map_err(|f| refused(&f))?;
        }
        Ok(picked)
    }

    /// A notice from 8 West (contract §6).
    async fn heard(&self, ledger: &Ledger, notice: &wire::Notice, picked: &mut PickedUp) {
        match notice.kind {
            NoticeType::ContactAccepted => {
                let Some(member_id) = notice.member_id.as_deref() else {
                    return;
                };
                if let Ok(Some(mut person)) = ledger.community_person(member_id) {
                    person.state = PersonState::Accepted;
                    person.updated_at = person.updated_at.max(self.now());
                    if ledger.community_put_person(&person).is_ok() {
                        self.record(
                            "community.conversation_accepted",
                            json!({ "name": person.name, "by": "them" }),
                        );
                    }
                }
            }
            NoticeType::StandingChanged | NoticeType::ProfileHidden => picked.me_changed = true,
            NoticeType::ReportClosed => {
                let outcome = notice.outcome.map_or("unknown", |o| match o {
                    wire::ReportOutcome::Action => "action",
                    wire::ReportOutcome::NoAction => "no_action",
                });
                self.record("community.report_closed", json!({ "outcome": outcome }));
            }
            // Your PCs: fetched fresh for each message, so nothing to do here.
            _ => {}
        }
    }

    /// Open one item, check it, and keep it. `Ok(false)`: already here.
    async fn open(
        &self,
        ledger: &Ledger,
        me: &Me,
        kind: ItemKind,
        arrived: &wire::InboxItem,
        pcs: &mut BTreeMap<String, Vec<DevicePublic>>,
    ) -> Result<bool, NotKept> {
        let unreadable = NotKept::Dropped("unreadable");
        let (Some(from), Some(sealed), Some(tag), Some(stamp)) = (
            arrived.from.as_deref(),
            arrived.sealed.as_deref(),
            arrived.tag.as_deref(),
            arrived.stamp.as_deref(),
        ) else {
            return Err(unreadable);
        };
        if !ids::is_id(IdKind::Member, from) {
            return Err(unreadable);
        }
        if !pcs.contains_key(from) {
            // A sender 8 West no longer lists (they left, or blocked you) can't be checked.
            let fetched = self.pcs_of(from).await.map_err(|f| {
                if for_now(&f) {
                    NotKept::Later
                } else {
                    NotKept::Dropped("unknown_pc")
                }
            })?;
            pcs.insert(from.to_owned(), fetched);
        }
        let sender_pcs = &pcs[from];
        let opened = item::open_item(
            &ThisPc {
                keys: me.pc.keys(),
                device_id: me.pc.device_id(),
                member_id: &me.member_id,
            },
            &Envelope {
                item_id: &arrived.item_id,
                kind,
                from,
                reference: arrived.reference.as_deref(),
                sealed,
                tag,
                stamp,
            },
            sender_pcs,
        )?;
        let payload = opened.payload;
        let body = match kind {
            ItemKind::Message => {
                serde_json::to_value(arrived_message(&payload.body).ok_or(unreadable)?)
            }
            _ => serde_json::to_value(arrived_reaction(&payload.body).ok_or(unreadable)?),
        }
        .map_err(|_| unreadable)?;

        // A copy of what you sent from another of your PCs is yours, in the conversation with
        // the person it went to.
        let outgoing = payload.from == me.member_id;
        let other = if outgoing { &payload.to } else { &payload.from };
        let mut person = match ledger.community_person(other) {
            Ok(Some(p)) => p,
            Ok(None) => {
                // Someone new: 8 West's list of your conversations names them.
                self.sync_conversations(ledger)
                    .await
                    .map_err(|_| NotKept::Later)?;
                ledger
                    .community_person(other)
                    .map_err(|_| NotKept::Later)?
                    .ok_or(NotKept::Dropped("unknown_person"))?
            }
            Err(_) => return Err(NotKept::Later),
        };
        if !outgoing {
            if arrived.request
                && matches!(
                    person.state,
                    PersonState::None | PersonState::LeftByMe | PersonState::LeftByThem
                )
            {
                person.state = PersonState::RequestedByThem;
                ledger
                    .community_put_person(&person)
                    .map_err(|_| NotKept::Later)?;
            }
            // Their PCs and yours, for the safety code.
            if let Ok(mine) = self.pcs_of(&me.member_id).await {
                self.keep_safety_code(ledger, me, &mine, &person.member_id, sender_pcs);
            }
        }
        let added = ledger
            .community_add_item(&CommunityItem {
                item_id: payload.item_id.clone(),
                member_id: person.member_id.clone(),
                outgoing,
                kind: match kind {
                    ItemKind::Reaction => KeptKind::Reaction,
                    _ => KeptKind::Message,
                },
                body,
                sent_at: payload.sent_at,
                accepted_at: Some(arrived.accepted_at),
                request: arrived.request,
                state: if outgoing {
                    ItemState::Delivered
                } else {
                    ItemState::Received
                },
                proof: (!outgoing).then(|| ItemProof {
                    payload: opened.kept.payload,
                    sig: opened.kept.sig,
                    fk: opened.kept.fk,
                    stamp: Some(opened.stamp),
                }),
                seen: outgoing,
            })
            .map_err(|_| NotKept::Later)?;
        Ok(added)
    }

    /// **Accept** someone's first message: you can now write to each other (contract §5).
    pub async fn accept(&self, ledger: &Ledger, member_id: &str) -> Result<(), Refused> {
        let request = client::accept_contact(member_id)
            .map_err(|_| Refused("That isn't someone in Community.".into()))?;
        self.as_member(&request, 204)
            .await
            .map_err(|f| refused(&f))?;
        if let Some(mut person) = ledger.community_person(member_id).map_err(kept)? {
            person.state = PersonState::Accepted;
            ledger.community_put_person(&person).map_err(kept)?;
            self.record(
                "community.conversation_accepted",
                json!({ "name": person.name, "by": "you" }),
            );
        }
        Ok(())
    }

    /// **Leave this conversation** (contract §5, ADR-167 §14): decline a request, take yours
    /// back, or stop getting their messages. Its messages are deleted from this PC (ADR-173).
    pub async fn leave_conversation(
        &self,
        ledger: &Ledger,
        member_id: &str,
    ) -> Result<(), Refused> {
        let request = client::leave_contact(member_id)
            .map_err(|_| Refused("That isn't someone in Community.".into()))?;
        self.as_member(&request, 204)
            .await
            .map_err(|f| refused(&f))?;
        if let Some(mut person) = ledger.community_person(member_id).map_err(kept)? {
            person.state = PersonState::LeftByMe;
            ledger.community_put_person(&person).map_err(kept)?;
            ledger
                .community_delete_conversation(member_id)
                .map_err(kept)?;
            self.record(
                "community.conversation_left",
                json!({ "name": person.name }),
            );
        }
        Ok(())
    }

    /// **Delete for me**: the message leaves this PC only (ADR-172). Nothing is sent.
    pub fn delete_for_me(&self, ledger: &Ledger, item_id: &str) -> Result<(), Refused> {
        if !ids::is_id(IdKind::Item, item_id) {
            return Err(Refused("That message isn't on this computer.".into()));
        }
        ledger.community_delete_item(item_id).map_err(kept)?;
        Ok(())
    }

    /// **Check the safety code**: you compared it, so "computers changed" goes away until they
    /// change again. Nothing is sent.
    pub fn safety_code_checked(&self, ledger: &Ledger, member_id: &str) -> Result<(), Refused> {
        let Some(mut person) = ledger.community_person(member_id).map_err(kept)? else {
            return Ok(());
        };
        person.safety_seen.clone_from(&person.safety_code);
        ledger.community_put_person(&person).map_err(kept)
    }
}

/// Your conversations, the newest first, and how many messages in each you haven't seen.
pub fn conversations(ledger: &Ledger) -> Result<Vec<ConversationSummary>, Refused> {
    let unseen: BTreeMap<String, u32> = ledger
        .community_unseen()
        .map_err(kept)?
        .into_iter()
        .collect();
    Ok(ledger
        .community_people()
        .map_err(kept)?
        .into_iter()
        .map(|p| summary_of(&p, unseen.get(&p.member_id).copied().unwrap_or(0)))
        .collect())
}

fn summary_of(p: &CommunityPerson, unseen: u32) -> ConversationSummary {
    ConversationSummary {
        member_id: p.member_id.clone(),
        name: p.name.clone(),
        display_name: p.display_name.clone(),
        state: state_word(p.state),
        unseen,
        last_at: p.updated_at,
        computers_changed: p.safety_seen.is_some() && p.safety_seen != p.safety_code,
    }
}

/// One conversation: up to 100 messages before `before` (`None`: the newest), with their
/// reactions. Opening it marks its messages seen.
pub fn conversation(
    ledger: &Ledger,
    member_id: &str,
    before: Option<i64>,
) -> Result<Option<ConversationView>, Refused> {
    let Some(person) = ledger.community_person(member_id).map_err(kept)? else {
        return Ok(None);
    };
    let items = ledger
        .community_items(member_id, before, 100)
        .map_err(kept)?;
    let reactions: Vec<&CommunityItem> = items
        .iter()
        .filter(|i| i.kind == KeptKind::Reaction)
        .collect();
    let messages = items
        .iter()
        .filter(|i| i.kind == KeptKind::Message)
        .map(|i| view_of(i, &reactions))
        .collect();
    ledger.community_mark_seen(member_id).map_err(kept)?;
    Ok(Some(ConversationView {
        person: summary_of(&person, 0),
        safety_code: person.safety_code.as_deref().map(grouped),
        messages,
    }))
}

/// A message as the screen shows it, with the latest reaction of each side.
fn view_of(item: &CommunityItem, reactions: &[&CommunityItem]) -> MessageView {
    let body: Option<wire::MessageBody> = serde_json::from_value(item.body.clone()).ok();
    let mut latest: BTreeMap<bool, (i64, Option<String>)> = BTreeMap::new();
    for r in reactions {
        let Ok(body) = serde_json::from_value::<wire::ReactionBody>(r.body.clone()) else {
            continue;
        };
        if body.item != item.item_id {
            continue;
        }
        let at = r.accepted_at.unwrap_or(r.sent_at);
        if latest.get(&r.outgoing).is_none_or(|(t, _)| *t <= at) {
            latest.insert(r.outgoing, (at, body.emoji));
        }
    }
    MessageView {
        item_id: item.item_id.clone(),
        outgoing: item.outgoing,
        text: body.as_ref().and_then(|b| b.text.clone()),
        has_gif: body.as_ref().is_some_and(|b| b.gif.is_some()),
        has_sticker: body.as_ref().is_some_and(|b| b.sticker.is_some()),
        reply_to: body.and_then(|b| b.reply_to),
        sent_at: item.sent_at,
        accepted_at: item.accepted_at,
        state: item.state.as_str().to_owned(),
        request: item.request,
        reactions: latest
            .into_iter()
            .filter_map(|(mine, (_, emoji))| emoji.map(|emoji| ReactionView { emoji, mine }))
            .collect(),
        reportable: !item.outgoing && item.proof.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_any_words_up_to_4000_characters() {
        assert!(message_text("   ").is_err());
        assert!(message_text(&"字".repeat(4000)).is_ok());
        assert!(message_text(&"字".repeat(4001)).is_err());
        assert_eq!(message_text("مرحبا 👋").unwrap(), "مرحبا 👋");
    }

    #[test]
    fn only_what_the_contract_allows_arrives() {
        let ok = json!({ "text": "Hi", "gif": null, "sticker": null, "reply_to": null });
        assert!(arrived_message(&ok).is_some());
        for bad in [
            json!({ "text": null, "gif": null, "sticker": null, "reply_to": null }),
            json!({ "text": "x".repeat(4001), "gif": null, "sticker": null, "reply_to": null }),
            json!({ "text": "Hi", "gif": null, "sticker": null, "reply_to": "../x" }),
            json!({ "text": "Hi", "gif": null, "sticker": null, "reply_to": null, "url": "x" }),
            json!({
                "text": null, "gif": { "library": "g", "id": "1" },
                "sticker": { "set": "s", "name": "n" }, "reply_to": null
            }),
        ] {
            assert!(arrived_message(&bad).is_none(), "{bad}");
        }
        let item = "ci_01J9Z8Y7X6W5V4T3S2R1Q0P9N8";
        assert!(arrived_reaction(&json!({ "item": item, "emoji": "👍" })).is_some());
        assert!(arrived_reaction(&json!({ "item": item, "emoji": null })).is_some());
        assert!(arrived_reaction(&json!({ "item": item, "emoji": "a b" })).is_none());
        assert!(arrived_reaction(&json!({ "item": item, "emoji": "x".repeat(17) })).is_none());
        assert!(arrived_reaction(&json!({ "item": "nope", "emoji": "👍" })).is_none());
    }

    #[test]
    fn the_safety_code_is_kept_as_digits_and_shown_in_groups() {
        assert_eq!(digits("5373 9207 7552"), "537392077552");
        assert_eq!(grouped("537392077552"), "5373 9207 7552");
    }
}
