//! Community's conversations and messages on this PC (Phase 24, ADR-164 §10, ADR-168 §3).
//!
//! Kept in the PC's shared record (the first organization's Ledger), in its backups and exports,
//! and never in Activity: nothing here makes an event, so no listener, notice, or log ever sees a
//! message's words. A message from someone else keeps its sealed proof (`payload`, `sig`, `fk`,
//! and the stamp), so it can be reported (ADR-164 §6). **Delete for me** removes a message on this
//! PC only (ADR-172); **Delete my Community data from this PC** removes them all.

use rusqlite::{params, Connection, OptionalExtension as _, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{LedgerError, Result};
use crate::Ledger;

/// Messages read at most at once.
pub const MOST_ITEMS: u32 = 200;

/// Where a conversation with one person stands (contract §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PersonState {
    /// Nothing yet: a card you looked at, or someone you wrote to who hasn't answered.
    None,
    RequestedByMe,
    /// Their first message waits in **Requests**.
    RequestedByThem,
    Accepted,
    LeftByMe,
    LeftByThem,
}

impl PersonState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RequestedByMe => "requestedByMe",
            Self::RequestedByThem => "requestedByThem",
            Self::Accepted => "accepted",
            Self::LeftByMe => "leftByMe",
            Self::LeftByThem => "leftByThem",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "none" => Self::None,
            "requestedByMe" => Self::RequestedByMe,
            "requestedByThem" => Self::RequestedByThem,
            "accepted" => Self::Accepted,
            "leftByMe" => Self::LeftByMe,
            "leftByThem" => Self::LeftByThem,
            _ => return None,
        })
    }
}

/// One person this PC talks with, or was asked by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityPerson {
    pub member_id: String,
    /// Their Community name, without the `@`.
    pub name: String,
    pub display_name: Option<String>,
    pub state: PersonState,
    /// The safety code now (12 digits), worked out from both people's PCs (ADR-164 §2).
    pub safety_code: Option<String>,
    /// The safety code as it was when you last looked: a different one says "Pat's computers
    /// changed".
    pub safety_seen: Option<String>,
    /// You blocked them: this PC refuses their items too (ADR-167 §4).
    pub blocked: bool,
    /// When anything last happened with them (Unix seconds).
    pub updated_at: i64,
}

/// The kind of a message on this PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Message,
    Reaction,
}

impl ItemKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Reaction => "reaction",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "message" => Some(Self::Message),
            "reaction" => Some(Self::Reaction),
            _ => None,
        }
    }
}

/// Where one message stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemState {
    /// Yours, not taken by 8 West yet: **Waiting to be delivered**.
    Waiting,
    /// Yours, taken by 8 West for their PCs: **Delivered**.
    Delivered,
    /// Yours, refused: they left the conversation, or have no PC signed in.
    NotDelivered,
    /// Theirs, picked up and checked by this PC.
    Received,
}

impl ItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Delivered => "delivered",
            Self::NotDelivered => "notDelivered",
            Self::Received => "received",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "waiting" => Self::Waiting,
            "delivered" => Self::Delivered,
            "notDelivered" => Self::NotDelivered,
            "received" => Self::Received,
            _ => return None,
        })
    }
}

/// What proves a message from someone else, for a report (ADR-164 §6): exactly as it came.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemProof {
    /// The item's inner JSON, base64url, exactly as signed.
    pub payload: String,
    pub sig: String,
    /// The report key.
    pub fk: String,
    pub stamp: Option<String>,
}

/// One message or reaction on this PC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityItem {
    pub item_id: String,
    /// The other person in the conversation.
    pub member_id: String,
    /// Sent by you (from this PC or another of yours).
    pub outgoing: bool,
    pub kind: ItemKind,
    /// The item's `body` (contract §7), as it was sent.
    pub body: Value,
    /// When it was written (Unix seconds, the sender's clock).
    pub sent_at: i64,
    /// When 8 West took it (Unix seconds), once it did.
    pub accepted_at: Option<i64>,
    /// It was a first message: a request.
    pub request: bool,
    pub state: ItemState,
    pub proof: Option<ItemProof>,
    /// You have seen it (theirs only).
    pub seen: bool,
}

fn bad_column(index: usize, text: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        format!("unknown value {text:?}").into(),
    )
}

fn person_of(r: &Row<'_>) -> rusqlite::Result<CommunityPerson> {
    let state: String = r.get(3)?;
    Ok(CommunityPerson {
        member_id: r.get(0)?,
        name: r.get(1)?,
        display_name: r.get(2)?,
        state: PersonState::parse(&state).ok_or_else(|| bad_column(3, &state))?,
        safety_code: r.get(4)?,
        safety_seen: r.get(5)?,
        blocked: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

const PERSON_COLUMNS: &str =
    "member_id, name, display_name, state, safety_code, safety_seen, blocked, updated_at";

fn item_of(r: &Row<'_>) -> rusqlite::Result<CommunityItem> {
    let kind: String = r.get(3)?;
    let body: String = r.get(4)?;
    let state: String = r.get(8)?;
    let payload: Option<String> = r.get(9)?;
    let sig: Option<String> = r.get(10)?;
    let fk: Option<String> = r.get(11)?;
    let stamp: Option<String> = r.get(12)?;
    Ok(CommunityItem {
        item_id: r.get(0)?,
        member_id: r.get(1)?,
        outgoing: r.get(2)?,
        kind: ItemKind::parse(&kind).ok_or_else(|| bad_column(3, &kind))?,
        body: serde_json::from_str(&body).map_err(|_| bad_column(4, "body"))?,
        sent_at: r.get(5)?,
        accepted_at: r.get(6)?,
        request: r.get(7)?,
        state: ItemState::parse(&state).ok_or_else(|| bad_column(8, &state))?,
        proof: match (payload, sig, fk) {
            (Some(payload), Some(sig), Some(fk)) => Some(ItemProof {
                payload,
                sig,
                fk,
                stamp,
            }),
            _ => None,
        },
        seen: r.get(13)?,
    })
}

const ITEM_COLUMNS: &str = "item_id, member_id, outgoing, kind, body, sent_at, accepted_at, \
                            request, state, payload, sig, fk, stamp, seen";

fn person(c: &Connection, member_id: &str) -> Result<Option<CommunityPerson>> {
    Ok(c.query_row(
        &format!("SELECT {PERSON_COLUMNS} FROM community_people WHERE member_id = ?1"),
        [member_id],
        person_of,
    )
    .optional()?)
}

impl Ledger {
    /// Keep a person, or what changed about them.
    pub fn community_put_person(&self, p: &CommunityPerson) -> Result<()> {
        self.write(|tx, _| {
            tx.execute(
                "INSERT INTO community_people
                     (member_id, name, display_name, state, safety_code, safety_seen, blocked,
                      updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (member_id) DO UPDATE SET
                     name = excluded.name, display_name = excluded.display_name,
                     state = excluded.state, safety_code = excluded.safety_code,
                     safety_seen = excluded.safety_seen, blocked = excluded.blocked,
                     updated_at = excluded.updated_at",
                params![
                    p.member_id,
                    p.name,
                    p.display_name,
                    p.state.as_str(),
                    p.safety_code,
                    p.safety_seen,
                    p.blocked,
                    p.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn community_person(&self, member_id: &str) -> Result<Option<CommunityPerson>> {
        self.read(|c| person(c, member_id))
    }

    /// Everyone, the most recent first.
    pub fn community_people(&self) -> Result<Vec<CommunityPerson>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {PERSON_COLUMNS} FROM community_people
                 ORDER BY updated_at DESC, member_id"
            ))?;
            let rows = stmt.query_map([], person_of)?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Keep a message, unless this PC already has it (the same item picked up twice, or a copy
    /// of your own from another of your PCs). `true` when it is new. The person must be kept
    /// first; their `updated_at` moves to the message's time.
    pub fn community_add_item(&self, item: &CommunityItem) -> Result<bool> {
        let body = serde_json::to_string(&item.body)?;
        self.write(|tx, _| {
            if person(tx, &item.member_id)?.is_none() {
                return Err(LedgerError::NotFound(format!(
                    "Community person {}",
                    item.member_id
                )));
            }
            let (payload, sig, fk, stamp) = match &item.proof {
                Some(p) => (
                    Some(p.payload.as_str()),
                    Some(p.sig.as_str()),
                    Some(p.fk.as_str()),
                    p.stamp.as_deref(),
                ),
                None => (None, None, None, None),
            };
            let added = tx.execute(
                "INSERT INTO community_items
                     (item_id, member_id, outgoing, kind, body, sent_at, accepted_at, request,
                      state, payload, sig, fk, stamp, seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT (item_id) DO NOTHING",
                params![
                    item.item_id,
                    item.member_id,
                    item.outgoing,
                    item.kind.as_str(),
                    body,
                    item.sent_at,
                    item.accepted_at,
                    item.request,
                    item.state.as_str(),
                    payload,
                    sig,
                    fk,
                    stamp,
                    item.seen,
                ],
            )? == 1;
            if added {
                tx.execute(
                    "UPDATE community_people SET updated_at = max(updated_at, ?2)
                     WHERE member_id = ?1",
                    params![item.member_id, item.accepted_at.unwrap_or(item.sent_at)],
                )?;
            }
            Ok(added)
        })
    }

    pub fn community_item(&self, item_id: &str) -> Result<Option<CommunityItem>> {
        self.read(|c| {
            Ok(c.query_row(
                &format!("SELECT {ITEM_COLUMNS} FROM community_items WHERE item_id = ?1"),
                [item_id],
                item_of,
            )
            .optional()?)
        })
    }

    /// The conversation with one person: up to `limit` items (at most 200) before `before`
    /// (Unix seconds; `None`: the newest), oldest first. They are in the order 8 West took them
    /// (the sender's clock for one not taken yet), then the order this PC kept them.
    pub fn community_items(
        &self,
        member_id: &str,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<CommunityItem>> {
        let limit = limit.clamp(1, MOST_ITEMS);
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {ITEM_COLUMNS} FROM community_items
                 WHERE member_id = ?1 AND coalesce(accepted_at, sent_at) < ?2
                 ORDER BY coalesce(accepted_at, sent_at) DESC, rowid DESC LIMIT ?3"
            ))?;
            let rows = stmt.query_map(
                params![member_id, before.unwrap_or(i64::MAX), limit],
                item_of,
            )?;
            let mut items: Vec<CommunityItem> = rows.collect::<rusqlite::Result<_>>()?;
            items.reverse();
            Ok(items)
        })
    }

    /// How many messages from each person you haven't seen.
    pub fn community_unseen(&self) -> Result<Vec<(String, u32)>> {
        self.read(|c| {
            let mut stmt = c.prepare(
                "SELECT member_id, count(*) FROM community_items
                 WHERE outgoing = 0 AND seen = 0 AND kind = 'message'
                 GROUP BY member_id",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// You saw the conversation with this person.
    pub fn community_mark_seen(&self, member_id: &str) -> Result<()> {
        self.write(|tx, _| {
            tx.execute(
                "UPDATE community_items SET seen = 1 WHERE member_id = ?1 AND seen = 0",
                [member_id],
            )?;
            Ok(())
        })
    }

    /// A message of yours after 8 West answered: taken (with when, and its stamp), or refused.
    pub fn community_set_item_state(
        &self,
        item_id: &str,
        state: ItemState,
        accepted_at: Option<i64>,
        stamp: Option<&str>,
    ) -> Result<()> {
        self.write(|tx, _| {
            let changed = tx.execute(
                "UPDATE community_items
                 SET state = ?2, accepted_at = coalesce(?3, accepted_at),
                     stamp = coalesce(?4, stamp)
                 WHERE item_id = ?1 AND outgoing = 1",
                params![item_id, state.as_str(), accepted_at, stamp],
            )?;
            if changed == 0 {
                return Err(LedgerError::NotFound(format!("your message {item_id}")));
            }
            Ok(())
        })
    }

    /// **Delete for me**: the message (and reactions to it) leave this PC. `true` when it was
    /// here.
    pub fn community_delete_item(&self, item_id: &str) -> Result<bool> {
        self.write(|tx, _| {
            let deleted =
                tx.execute("DELETE FROM community_items WHERE item_id = ?1", [item_id])?;
            tx.execute(
                "DELETE FROM community_items
                 WHERE kind = 'reaction' AND json_extract(body, '$.item') = ?1",
                [item_id],
            )?;
            Ok(deleted == 1)
        })
    }

    /// **Leave this conversation**: its messages leave this PC; the person stays, so this PC knows
    /// you left (ADR-173).
    pub fn community_delete_conversation(&self, member_id: &str) -> Result<()> {
        self.write(|tx, _| {
            tx.execute(
                "DELETE FROM community_items WHERE member_id = ?1",
                [member_id],
            )?;
            Ok(())
        })
    }

    /// **Delete my Community data from this PC**: every conversation and message. The events
    /// about Community stay in Activity; they never held a message's words.
    pub fn community_delete_all(&self) -> Result<()> {
        self.write(|tx, _| {
            tx.execute_batch("DELETE FROM community_items; DELETE FROM community_people;")?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PAT: &str = "cm_01J9Z8Y7X6W5V4T3S2R1Q0P9N8";

    fn pat() -> CommunityPerson {
        CommunityPerson {
            member_id: PAT.into(),
            name: "pat-lee".into(),
            display_name: Some("Pat Lee".into()),
            state: PersonState::RequestedByThem,
            safety_code: Some("537392077552".into()),
            safety_seen: None,
            blocked: false,
            updated_at: 100,
        }
    }

    fn item(id: &str, sent_at: i64, outgoing: bool) -> CommunityItem {
        CommunityItem {
            item_id: id.into(),
            member_id: PAT.into(),
            outgoing,
            kind: ItemKind::Message,
            body: json!({ "text": "Hi there", "gif": null, "sticker": null, "reply_to": null }),
            sent_at,
            accepted_at: Some(sent_at + 1),
            request: false,
            state: if outgoing {
                ItemState::Delivered
            } else {
                ItemState::Received
            },
            proof: (!outgoing).then(|| ItemProof {
                payload: "eyJ2IjoxfQ".into(),
                sig: "c2ln".into(),
                fk: "Zms".into(),
                stamp: Some("a.b".into()),
            }),
            seen: outgoing,
        }
    }

    fn id(n: u32) -> String {
        format!("ci_01J9Z8Y7X6W5V4T3S2R1Q0P{n:03}")
    }

    #[test]
    fn a_conversation_is_kept_oldest_first_and_never_twice() {
        let l = Ledger::open_in_memory().unwrap();
        assert!(
            l.community_add_item(&item(&id(1), 200, false)).is_err(),
            "the person comes first"
        );
        l.community_put_person(&pat()).unwrap();
        assert!(l.community_add_item(&item(&id(1), 200, false)).unwrap());
        assert!(!l.community_add_item(&item(&id(1), 200, false)).unwrap());
        assert!(l.community_add_item(&item(&id(2), 300, true)).unwrap());
        let items = l.community_items(PAT, None, 50).unwrap();
        assert_eq!(
            items.iter().map(|i| i.item_id.clone()).collect::<Vec<_>>(),
            [id(1), id(2)]
        );
        assert_eq!(
            items[0].proof.as_ref().unwrap().stamp.as_deref(),
            Some("a.b")
        );
        assert_eq!(items[0].body["text"], "Hi there");
        assert_eq!(l.community_items(PAT, Some(300), 50).unwrap().len(), 1);
        assert_eq!(l.community_person(PAT).unwrap().unwrap().updated_at, 301);
        assert_eq!(l.community_unseen().unwrap(), [(PAT.to_owned(), 1)]);
        l.community_mark_seen(PAT).unwrap();
        assert!(l.community_unseen().unwrap().is_empty());
    }

    #[test]
    fn a_message_never_becomes_an_event() {
        let l = Ledger::open_in_memory().unwrap();
        let heard = std::sync::Arc::new(std::sync::Mutex::new(0));
        let h = heard.clone();
        l.add_listener(std::sync::Arc::new(move |_| *h.lock().unwrap() += 1));
        l.community_put_person(&pat()).unwrap();
        l.community_add_item(&item(&id(1), 200, false)).unwrap();
        l.community_add_item(&item(&id(2), 210, true)).unwrap();
        l.community_set_item_state(&id(2), ItemState::NotDelivered, None, None)
            .unwrap();
        l.community_delete_item(&id(1)).unwrap();
        assert_eq!(*heard.lock().unwrap(), 0);
    }

    #[test]
    fn delete_for_me_takes_its_reactions_and_delete_all_takes_everything() {
        let l = Ledger::open_in_memory().unwrap();
        l.community_put_person(&pat()).unwrap();
        l.community_add_item(&item(&id(1), 200, false)).unwrap();
        let mut reaction = item(&id(2), 210, true);
        reaction.kind = ItemKind::Reaction;
        reaction.body = json!({ "item": id(1), "emoji": "👍" });
        l.community_add_item(&reaction).unwrap();
        l.community_add_item(&item(&id(3), 220, true)).unwrap();
        assert!(l.community_delete_item(&id(1)).unwrap());
        assert!(!l.community_delete_item(&id(1)).unwrap());
        let left: Vec<_> = l
            .community_items(PAT, None, 50)
            .unwrap()
            .into_iter()
            .map(|i| i.item_id)
            .collect();
        assert_eq!(left, [id(3)]);
        l.community_add_item(&item(&id(4), 230, false)).unwrap();
        l.community_delete_conversation(PAT).unwrap();
        assert!(l.community_items(PAT, None, 50).unwrap().is_empty());
        assert!(
            l.community_person(PAT).unwrap().is_some(),
            "the person stays"
        );
        l.community_delete_all().unwrap();
        assert!(l.community_people().unwrap().is_empty());
        assert!(l.community_item(&id(3)).unwrap().is_none());
    }

    #[test]
    fn only_your_own_message_takes_8_wests_answer() {
        let l = Ledger::open_in_memory().unwrap();
        l.community_put_person(&pat()).unwrap();
        l.community_add_item(&item(&id(1), 200, false)).unwrap();
        let mut mine = item(&id(2), 210, true);
        mine.state = ItemState::Waiting;
        mine.accepted_at = None;
        l.community_add_item(&mine).unwrap();
        assert!(l
            .community_set_item_state(&id(1), ItemState::Delivered, Some(5), None)
            .is_err());
        l.community_set_item_state(&id(2), ItemState::Delivered, Some(215), Some("s.t"))
            .unwrap();
        let kept = l.community_item(&id(2)).unwrap().unwrap();
        assert_eq!(kept.state, ItemState::Delivered);
        assert_eq!(kept.accepted_at, Some(215));
    }

    #[test]
    fn what_is_not_an_id_a_name_or_a_code_is_refused() {
        let l = Ledger::open_in_memory().unwrap();
        let mut bad = pat();
        bad.member_id = "../x".into();
        assert!(l.community_put_person(&bad).is_err());
        let mut bad = pat();
        bad.safety_code = Some("5373 9207 75".into());
        assert!(l.community_put_person(&bad).is_err());
        let mut bad = pat();
        bad.display_name = Some("x".repeat(61));
        assert!(l.community_put_person(&bad).is_err());
        l.community_put_person(&pat()).unwrap();
        let mut no_id = item("ci_short", 200, false);
        no_id.proof = None;
        assert!(l.community_add_item(&no_id).is_err());
    }
}
