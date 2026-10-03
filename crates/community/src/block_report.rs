//! **Block**, **Unblock**, **Report**, and **Delete my Community data from this PC** (ADR-167,
//! ADR-168 §3).
//!
//! - A block works everywhere at once, and twice: 8 West enforces it, and this PC refuses the
//!   blocked person's items too (ADR-167 §4). They are not told.
//! - A report carries what you report and nothing else: for messages, the ones you tick (up to
//!   20), each with the proof that 8 West took it from them (ADR-164 §6). Never your files, your
//!   Ledger, or other conversations (ADR-167 §6).
//! - The Ledger records that you blocked, unblocked, or reported someone, and why, by their
//!   Community name; never a note or a message's words (ADR-167 §16).

use plenipo_ledger::community::{CommunityPerson, PersonState};
use plenipo_ledger::Ledger;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::client::{self, Failure, Transport};
use crate::ids::{self, IdKind};
use crate::service::{words, Community, Refused};
use crate::wire::{self, ReportReason, ReportWhat};

/// The most messages in one report (contract §11).
pub const MOST_REPORTED_ITEMS: usize = 20;
/// The longest note on a report (contract §11).
pub const MOST_NOTE_CHARS: usize = 1000;

/// One person you blocked, for Settings → Community → **Blocked**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlockedPerson {
    pub member_id: String,
    /// Their Community name, without the `@`.
    pub name: String,
    /// When you blocked them (Unix seconds).
    #[ts(type = "number")]
    pub blocked_at: i64,
}

/// What a report is about, as the screen asks for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ReportOf {
    /// The person.
    Person,
    /// Their profile: 8 West keeps a copy of it as it is now.
    Profile,
    /// Messages they sent you, ticked on screen (1 to 20).
    #[serde(rename_all = "camelCase")]
    Messages { item_ids: Vec<String> },
}

fn kept(e: plenipo_ledger::LedgerError) -> Refused {
    log::warn!("Community could not use this PC's record: {e}");
    Refused("Plenipo couldn't save this on this computer.".into())
}

fn refused(failure: &Failure) -> Refused {
    Refused(words(failure))
}

/// A reason's word, as the contract writes it (for the Ledger).
fn reason_word(reason: ReportReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A report's note: up to 1,000 characters, or none.
fn note_of(note: &str) -> Result<Option<String>, Refused> {
    let note = note.trim();
    if note.is_empty() {
        return Ok(None);
    }
    if note.chars().count() > MOST_NOTE_CHARS {
        return Err(Refused(format!(
            "A report's note can be at most {MOST_NOTE_CHARS} characters."
        )));
    }
    Ok(Some(note.to_owned()))
}

impl<T: Transport> Community<T> {
    /// **Block** someone: they can't message you, find your card, or link with you, and any link
    /// or collaboration with them ends. They are not told. What is on this PC stays.
    pub async fn block(&self, ledger: &Ledger, member_id: &str, name: &str) -> Result<(), Refused> {
        let request = client::block(member_id)
            .map_err(|_| Refused("That isn't someone in Community.".into()))?;
        if !client::is_community_name(name) {
            return Err(Refused("That isn't someone in Community.".into()));
        }
        self.as_member(&request, 204)
            .await
            .map_err(|f| refused(&f))?;
        let mut person = ledger
            .community_person(member_id)
            .map_err(kept)?
            .unwrap_or_else(|| CommunityPerson {
                member_id: member_id.to_owned(),
                name: name.to_owned(),
                display_name: None,
                state: PersonState::None,
                safety_code: None,
                safety_seen: None,
                blocked: false,
                updated_at: self.now(),
            });
        person.blocked = true;
        // 8 West ended the conversation: writing again would be a new request (contract §8).
        if matches!(
            person.state,
            PersonState::Accepted | PersonState::RequestedByMe | PersonState::RequestedByThem
        ) {
            person.state = PersonState::LeftByMe;
        }
        ledger.community_put_person(&person).map_err(kept)?;
        self.record("community.blocked", json!({ "name": person.name }));
        Ok(())
    }

    /// **Unblock** someone. Links and collaborations the block ended don't come back.
    pub async fn unblock(&self, ledger: &Ledger, member_id: &str) -> Result<(), Refused> {
        let request = client::unblock(member_id)
            .map_err(|_| Refused("That isn't someone in Community.".into()))?;
        self.as_member(&request, 204)
            .await
            .map_err(|f| refused(&f))?;
        if let Some(mut person) = ledger.community_person(member_id).map_err(kept)? {
            person.blocked = false;
            ledger.community_put_person(&person).map_err(kept)?;
            self.record("community.unblocked", json!({ "name": person.name }));
        }
        Ok(())
    }

    /// Settings → Community → **Blocked**: everyone you blocked, as 8 West has it, and this PC's
    /// marks brought up to date with it.
    pub async fn blocked(&self, ledger: &Ledger) -> Result<Vec<BlockedPerson>, Refused> {
        let answer = self
            .as_member(&client::blocks(), 200)
            .await
            .map_err(|f| refused(&f))?;
        let blocks: wire::Blocks = client::read(answer, 200).map_err(|f| refused(&f))?;
        let blocks: Vec<wire::Block> = blocks
            .blocks
            .into_iter()
            .filter(|b| {
                ids::is_id(IdKind::Member, &b.member_id) && client::is_community_name(&b.name)
            })
            .collect();
        for person in ledger.community_people().map_err(kept)? {
            let blocked = blocks.iter().any(|b| b.member_id == person.member_id);
            if person.blocked != blocked {
                let mut person = person;
                person.blocked = blocked;
                ledger.community_put_person(&person).map_err(kept)?;
            }
        }
        for block in &blocks {
            if ledger
                .community_person(&block.member_id)
                .map_err(kept)?
                .is_none()
            {
                ledger
                    .community_put_person(&CommunityPerson {
                        member_id: block.member_id.clone(),
                        name: block.name.clone(),
                        display_name: None,
                        state: PersonState::None,
                        safety_code: None,
                        safety_seen: None,
                        blocked: true,
                        updated_at: block.blocked_at,
                    })
                    .map_err(kept)?;
            }
        }
        Ok(blocks
            .into_iter()
            .map(|b| BlockedPerson {
                member_id: b.member_id,
                name: b.name,
                blocked_at: b.blocked_at,
            })
            .collect())
    }

    /// **Report** someone, their profile, or messages they sent you (ADR-167 §5, §6). A message
    /// goes with its proof exactly as it came, so 8 West can tell it is real; a message without
    /// one (yours, or one deleted from this PC) can't be reported from here.
    pub async fn report(
        &self,
        ledger: &Ledger,
        about: &str,
        of: &ReportOf,
        reason: ReportReason,
        note: &str,
    ) -> Result<(), Refused> {
        if !ids::is_id(IdKind::Member, about) {
            return Err(Refused("That isn't someone in Community.".into()));
        }
        let note = note_of(note)?;
        let (what, items) = match of {
            ReportOf::Person => (ReportWhat::Person, Vec::new()),
            ReportOf::Profile => (ReportWhat::Profile, Vec::new()),
            ReportOf::Messages { item_ids } => {
                if item_ids.is_empty() || item_ids.len() > MOST_REPORTED_ITEMS {
                    return Err(Refused(format!(
                        "Tick between 1 and {MOST_REPORTED_ITEMS} messages to report."
                    )));
                }
                let mut items = Vec::with_capacity(item_ids.len());
                for item_id in item_ids {
                    let proof = ledger
                        .community_item(item_id)
                        .map_err(kept)?
                        .filter(|i| !i.outgoing && i.member_id == about)
                        .and_then(|i| i.proof)
                        .ok_or_else(|| {
                            Refused("Only messages from this person, still on this computer, can be reported.".into())
                        })?;
                    let stamp = proof.stamp.ok_or_else(|| {
                        Refused("Only messages from this person, still on this computer, can be reported.".into())
                    })?;
                    items.push(wire::ReportItem {
                        stamp,
                        payload: proof.payload,
                        fk: proof.fk,
                    });
                }
                (ReportWhat::Items, items)
            }
        };
        let body = wire::Report {
            about: about.to_owned(),
            reason,
            note,
            what,
            items,
        };
        let answer = self
            .as_member(&client::report(&body), 200)
            .await
            .map_err(|f| refused(&f))?;
        let _made: wire::ReportMade = client::read(answer, 200).map_err(|f| refused(&f))?;
        let name = ledger
            .community_person(about)
            .ok()
            .flatten()
            .map(|p| p.name);
        let what = match of {
            ReportOf::Person => "person",
            ReportOf::Profile => "profile",
            ReportOf::Messages { .. } => "messages",
        };
        self.record(
            "community.reported",
            json!({ "name": name, "what": what, "reason": reason_word(reason) }),
        );
        Ok(())
    }

    /// **Delete my Community data from this PC** (ADR-168 §3): every conversation and message
    /// here. Nothing is sent; 8 West and the other people keep theirs. The Ledger's events about
    /// Community stay with your other Activity; they never held a message's words.
    pub fn delete_my_data(&self, ledger: &Ledger) -> Result<(), Refused> {
        ledger.community_delete_all().map_err(kept)?;
        self.record("community.data_deleted", json!({}));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_up_to_1000_characters_or_none() {
        assert_eq!(note_of("   ").unwrap(), None);
        assert_eq!(
            note_of(" spam links ").unwrap().as_deref(),
            Some("spam links")
        );
        assert!(note_of(&"字".repeat(1000)).is_ok());
        assert!(note_of(&"字".repeat(1001)).is_err());
    }

    #[test]
    fn a_reason_is_written_as_the_contract_does() {
        assert_eq!(reason_word(ReportReason::Under13), "under_13");
        assert_eq!(
            reason_word(ReportReason::YoungPersonRisk),
            "young_person_risk"
        );
    }
}
