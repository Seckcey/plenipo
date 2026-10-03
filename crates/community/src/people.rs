//! Finding people (ADR-163 §4, §6): the Community directory, **New this week**, **Find someone**,
//! a card, a picture, **Invite by email**, and **Share my profile**.
//!
//! Everything here is what other people wrote about themselves: the screen shows it as plain text
//! (ADR-164 §4). A picture is checked to be a real PNG of at most 256 × 256 before the screen gets
//! it, and Plenipo fetches it only from 8 West's own address, through Guard: never from an address
//! someone wrote.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{self, ErrorCode, Failure, Transport};
use crate::service::{words, Community, Refused};
use crate::wire;

/// The longest search (contract §4).
const MOST_SEARCH_CHARS: usize = 60;
/// The largest picture, in pixels on each side and in bytes (contract §15).
const MOST_PICTURE_SIDE: u32 = 256;
const MOST_PICTURE_BYTES: usize = 256 * 1024;
/// The longest email address (RFC 5321).
const MOST_EMAIL_CHARS: usize = 254;
/// Where **Share my profile** points: one plain page on Plenipo's website, the same for every
/// name, that never checks whether the name exists (ADR-163 §6).
pub const SHARE_PAGE: &str = "https://getplenipo.com/c/";

/// One member's card, as the screen shows it. Parts the member hides are `null` (or empty).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardView {
    pub member_id: String,
    /// The Community name, without the `@`.
    pub name: String,
    pub display_name: Option<String>,
    /// `available`, `busy`, `away`, or `offline`; `null` when it isn't shown.
    pub status: Option<String>,
    /// The mood's word (`great`, `focused`, …), or `null`.
    pub mood: Option<String>,
    pub message: Option<String>,
    pub company: Option<String>,
    /// Up to 3 kinds of business, as the contract names them (`construction`, …).
    pub business_kinds: Vec<String>,
    pub business_line: Option<String>,
    /// A country (`US`) or a state (`US-CA`).
    pub region: Option<String>,
    pub has_picture: bool,
    /// Changes when the picture does, so the screen asks for it again only then.
    pub picture_version: Option<String>,
    /// The badges this copy of Plenipo knows (ADR-169 §3).
    pub badges: Vec<String>,
    /// Points, all time.
    #[ts(type = "number")]
    pub points: i64,
    /// How many different people thanked this member.
    pub thanked_by: u32,
}

/// A page of cards: 20 at a time, and where the next page starts, or `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PeoplePage {
    pub cards: Vec<CardView>,
    pub next: Option<String>,
}

/// What **Find someone** found for an exact Community name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Found {
    /// Their card.
    #[serde(rename_all = "camelCase")]
    Card { card: CardView },
    /// A member under 18, or one who appears offline: only **Send a message request**.
    #[serde(rename_all = "camelCase")]
    RequestOnly { member_id: String, name: String },
    /// Nobody to show by that name (or nobody at all: the two look the same, on purpose).
    NoOne,
}

/// A word the contract writes in `snake_case`, as the screen uses it.
fn word<V: Serialize>(value: &V) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

impl CardView {
    /// The card as the screen shows it. Words this copy does not know yet are left out.
    pub fn of(card: wire::Card) -> Self {
        Self {
            member_id: card.member_id,
            name: card.name,
            display_name: card.display_name,
            status: card
                .status
                .filter(|s| *s != wire::CardStatus::Unknown)
                .and_then(|s| word(&s)),
            mood: card.mood.and_then(|m| word(&m)),
            message: card.message,
            company: card.company,
            business_kinds: card.business_kinds.iter().filter_map(word).collect(),
            business_line: card.business_line,
            region: card.region,
            has_picture: card.has_picture,
            picture_version: card.picture_version,
            badges: card
                .badges
                .iter()
                .filter(|b| **b != wire::Badge::Unknown)
                .filter_map(word)
                .collect(),
            points: card.points,
            thanked_by: card.thanked_by,
        }
    }
}

/// A search's words: one line, trimmed, at most 60 characters.
fn search_words(q: &str) -> Result<String, Refused> {
    let q = q.trim();
    if q.chars().any(char::is_control) {
        return Err(Refused("Search for one line of words.".into()));
    }
    if q.chars().count() > MOST_SEARCH_CHARS {
        return Err(Refused(format!(
            "Search for at most {MOST_SEARCH_CHARS} characters."
        )));
    }
    Ok(q.to_owned())
}

/// A word of the contract's (a business kind) or a place (`US`, `US-CA`), or nothing.
fn plain_word(text: &str, what: &str) -> Result<String, Refused> {
    let text = text.trim();
    let fits = text.len() <= 32
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if fits {
        Ok(text.to_owned())
    } else {
        Err(Refused(format!("That isn't {what}.")))
    }
}

/// Whether `bytes` is a PNG of at most 256 × 256 pixels and 256 KB, every pixel of which reads.
pub fn is_safe_picture(bytes: &[u8]) -> bool {
    if bytes.len() > MOST_PICTURE_BYTES {
        return false;
    }
    let Ok(mut reader) = png::Decoder::new(std::io::Cursor::new(bytes)).read_info() else {
        return false;
    };
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > MOST_PICTURE_SIDE
        || info.height > MOST_PICTURE_SIDE
    {
        return false;
    }
    let Some(size) = reader.output_buffer_size() else {
        return false;
    };
    let mut frame = vec![0; size];
    reader.next_frame(&mut frame).is_ok()
}

/// Whether `email` looks like one email address: something, `@`, and a domain with a dot. 8 West
/// checks it properly; this only keeps a typo or a list from being sent.
fn looks_like_an_email(email: &str) -> bool {
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.chars().count() <= MOST_EMAIL_CHARS
        && !email
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == ',' || c == ';')
        && !local.contains('@')
}

/// The link **Share my profile** gives for a Community name.
pub fn share_link(name: &str) -> Option<String> {
    client::is_community_name(name).then(|| format!("{SHARE_PAGE}{name}"))
}

impl<T: Transport> Community<T> {
    /// The Community directory (ADR-163 §4): adults listed, most points first, 20 at a time.
    /// `q` matches a name, a company, or a line about a business; `kind` is one business kind;
    /// `region` a country or a state. `cursor` is where the page starts (empty: the first).
    pub async fn directory(
        &self,
        q: &str,
        kind: &str,
        region: &str,
        cursor: &str,
    ) -> Result<PeoplePage, Refused> {
        let q = search_words(q)?;
        let kind = plain_word(kind, "a kind of business")?;
        let region = plain_word(region, "a state or a country")?;
        let cursor = plain_word(cursor, "a page")?;
        self.page(&client::directory(false, &q, &kind, &region, &cursor))
            .await
    }

    /// **New this week** (ADR-163 §4): people who joined in the last 7 days.
    pub async fn new_this_week(&self, cursor: &str) -> Result<PeoplePage, Refused> {
        let cursor = plain_word(cursor, "a page")?;
        self.page(&client::directory(true, "", "", "", &cursor))
            .await
    }

    async fn page(&self, request: &client::Request) -> Result<PeoplePage, Refused> {
        let answer = self
            .as_member(request, 200)
            .await
            .map_err(|f| Refused(words(&f)))?;
        let page: wire::CardPage = client::read(answer, 200).map_err(|f| Refused(words(&f)))?;
        Ok(PeoplePage {
            cards: page.cards.into_iter().map(CardView::of).collect(),
            next: page.next,
        })
    }

    /// **Find someone** by their exact Community name (ADR-163 §6). Nobody, and a member who
    /// blocked you, look the same.
    pub async fn find(&self, name: &str) -> Result<Found, Refused> {
        let name = name.trim().trim_start_matches('@').to_lowercase();
        let Ok(request) = client::by_name(&name) else {
            return Err(Refused(
                "A Community name is 3 to 30 letters, numbers, or dashes.".into(),
            ));
        };
        let answer = match self.as_member(&request, 200).await {
            Ok(answer) => answer,
            Err(failure) if failure.code() == Some(&ErrorCode::NotFound) => {
                return Ok(Found::NoOne);
            }
            Err(failure) => return Err(Refused(words(&failure))),
        };
        let value: serde_json::Value = client::read(answer, 200).map_err(|f| Refused(words(&f)))?;
        if value.get("request_only") == Some(&serde_json::Value::Bool(true)) {
            let only: wire::RequestOnly =
                serde_json::from_value(value).map_err(|_| Refused(words(&Failure::BadAnswer)))?;
            return Ok(Found::RequestOnly {
                member_id: only.member_id,
                name: only.name,
            });
        }
        let card: wire::Card =
            serde_json::from_value(value).map_err(|_| Refused(words(&Failure::BadAnswer)))?;
        Ok(Found::Card {
            card: CardView::of(card),
        })
    }

    /// One member's card, when you may see it (ADR-163 §7); `None` when you may not.
    pub async fn card(&self, member_id: &str) -> Result<Option<CardView>, Refused> {
        let Ok(request) = client::person(member_id) else {
            return Ok(None);
        };
        match self.as_member(&request, 200).await {
            Ok(answer) => {
                let card: wire::Card = client::read(answer, 200).map_err(|f| Refused(words(&f)))?;
                Ok(Some(CardView::of(card)))
            }
            Err(failure) if failure.code() == Some(&ErrorCode::NotFound) => Ok(None),
            Err(failure) => Err(Refused(words(&failure))),
        }
    }

    /// One member's picture: a PNG of at most 256 × 256, checked here, or `None`.
    pub async fn picture(&self, member_id: &str) -> Option<Vec<u8>> {
        let request = client::picture_of(member_id).ok()?;
        let answer = self.as_member(&request, 200).await.ok()?;
        is_safe_picture(&answer.body).then_some(answer.body)
    }

    /// **Invite by email** (ADR-163 §6): 8 West emails the address a link to join. The answer is
    /// the same whether or not the address has an account. The Ledger records that you invited
    /// someone, never the address.
    pub async fn invite_by_email(&self, email: &str) -> Result<(), Refused> {
        let email = email.trim();
        if !looks_like_an_email(email) {
            return Err(Refused("Type one email address.".into()));
        }
        let body = wire::InviteEmail {
            email: email.to_owned(),
        };
        self.as_member(&client::invite_email(&body), 202)
            .await
            .map_err(|f| Refused(words(&f)))?;
        self.record("community.invited", serde_json::json!({}));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_is_one_short_line_and_a_kind_or_place_is_one_word() {
        assert_eq!(search_words("  plumbers  ").unwrap(), "plumbers");
        assert!(search_words(&"a".repeat(61)).is_err());
        assert!(search_words("two\nlines").is_err());
        assert_eq!(plain_word("US-CA", "").unwrap(), "US-CA");
        assert_eq!(plain_word("it_services", "").unwrap(), "it_services");
        assert!(plain_word("US&kind=legal", "").is_err());
        assert!(plain_word("../me", "").is_err());
    }

    #[test]
    fn only_one_plain_email_address_is_sent() {
        assert!(looks_like_an_email("pat@example.com"));
        assert!(looks_like_an_email("pat+tag@sub.example.co.uk"));
        for bad in [
            "pat",
            "pat@",
            "@example.com",
            "pat@example",
            "pat@.com",
            "pat@example.",
            "pat@example.com, sam@example.com",
            "pat lee@example.com",
            "pat@@example.com",
            "pat@example.com\n",
        ] {
            assert!(!looks_like_an_email(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_picture_must_be_a_small_real_png() {
        let ok = crate::stand_in::tiny_png(16, 16);
        assert!(is_safe_picture(&ok));
        assert!(!is_safe_picture(&crate::stand_in::tiny_png(257, 16)));
        assert!(!is_safe_picture(b"<svg onload=alert(1)>"));
        let mut broken = ok.clone();
        broken.truncate(ok.len() - 20);
        assert!(!is_safe_picture(&broken));
    }

    #[test]
    fn the_share_link_is_the_same_page_for_every_name() {
        assert_eq!(
            share_link("frank-g").as_deref(),
            Some("https://getplenipo.com/c/frank-g")
        );
        assert_eq!(share_link("../admin"), None);
        assert_eq!(share_link("Frank"), None);
    }

    #[test]
    fn words_this_copy_does_not_know_are_left_off_a_card() {
        let card: wire::Card = serde_json::from_value(serde_json::json!({
            "member_id": "cm_01J9Z8Y7X6W5V4T3S2R1Q0P9N8",
            "name": "pat-lee",
            "display_name": "Pat Lee",
            "status": "on_the_moon",
            "mood": "focused",
            "message": null,
            "company": "Lee Builders",
            "business_kinds": ["construction"],
            "business_line": null,
            "region": "US-CA",
            "has_picture": false,
            "picture_version": null,
            "badges": ["helper", "future_badge"],
            "points": 42,
            "thanked_by": 3
        }))
        .unwrap();
        let view = CardView::of(card);
        assert_eq!(view.status, None);
        assert_eq!(view.mood.as_deref(), Some("focused"));
        assert_eq!(view.badges, ["helper"]);
        assert_eq!(view.business_kinds, ["construction"]);
    }
}
