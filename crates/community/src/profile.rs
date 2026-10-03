//! Your profile in Community (ADR-163 §1, §2, §8): your tile (picture, status, mood, and message)
//! and what you add (your name, company, what your business does, and where), each part with its
//! own box, all ticked to begin with. What is unticked or empty is never sent. **Do not disturb**
//! shows to others as **Busy**.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::service::Refused;
use crate::wire;

/// The longest name on a card (contract §3).
const MOST_NAME_CHARS: usize = 60;
/// The longest message, company, or line about the business (contract §3).
const MOST_LINE_CHARS: usize = 80;
/// The most kinds of business (contract §3).
const MOST_KINDS: usize = 3;

/// Which parts of your profile people see (ADR-163 §2). All ticked to begin with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileShown {
    pub picture: bool,
    pub name: bool,
    pub status: bool,
    pub mood: bool,
    pub message: bool,
    pub company: bool,
    /// What your business does: its kinds and your line about it.
    pub business: bool,
    pub region: bool,
}

impl Default for ProfileShown {
    fn default() -> Self {
        Self {
            picture: true,
            name: true,
            status: true,
            mood: true,
            message: true,
            company: true,
            business: true,
            region: true,
        }
    }
}

/// What you add to your tile for Community, and which parts are shown. Kept on this computer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ProfileDraft {
    /// Your name on your card: your account's name to begin with.
    pub display_name: String,
    pub company: String,
    /// Up to 3 kinds of business, as the contract names them (`construction`, `accounting`, …).
    pub business_kinds: Vec<String>,
    /// What your business does, in your own words.
    pub business_line: String,
    /// A country (`US`) or a US state (`US-CA`); never a town or an address. Empty: not shown.
    pub region: String,
    pub shown: ProfileShown,
}

/// Your tile's status light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TileStatus {
    Available,
    Busy,
    Away,
    DoNotDisturb,
}

/// Your tile, as Community uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub status: TileStatus,
    /// The mood's word (`great`, `focused`, …), or none.
    pub mood: Option<String>,
    pub message: String,
    /// The picture, a PNG already checked by the tile (256 × 256 at most), or none.
    pub picture: Option<Vec<u8>>,
}

/// The largest picture a tile keeps, in bytes (ADR-056).
const MOST_PICTURE_BYTES: usize = 256 * 1024;

impl TileStatus {
    /// The status as the tile keeps it (`available`, `busy`, `away`, `doNotDisturb`).
    pub fn from_word(word: &str) -> Self {
        match word {
            "busy" => Self::Busy,
            "away" => Self::Away,
            "doNotDisturb" => Self::DoNotDisturb,
            _ => Self::Available,
        }
    }
}

/// The tile's picture, kept as standard base64, as bytes; `None` if it is not one.
pub fn picture_from_base64(text: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    if text.len() > MOST_PICTURE_BYTES.div_ceil(3) * 4 + 4 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .ok()?;
    (bytes.len() <= MOST_PICTURE_BYTES).then_some(bytes)
}

/// One line of text from you, for a card: trimmed, no control characters, and not too long.
fn line(text: &str, most: usize, what: &str) -> Result<Option<String>, Refused> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().any(char::is_control) {
        return Err(Refused(format!("{what} must be one line.")));
    }
    if text.chars().count() > most {
        return Err(Refused(format!("{what} can be at most {most} characters.")));
    }
    Ok(Some(text.to_owned()))
}

/// Whether `region` is a country (`US`) or a state (`US-CA`), as the contract writes them.
fn is_region(region: &str) -> bool {
    let bytes = region.as_bytes();
    let country = bytes.len() >= 2 && bytes[..2].iter().all(u8::is_ascii_uppercase);
    match bytes.len() {
        2 => country,
        4..=6 => {
            country
                && bytes[2] == b'-'
                && bytes[3..]
                    .iter()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        }
        _ => false,
    }
}

/// The profile to send: the parts shown, from your tile and what you added. Parts unticked or
/// empty are `null`, so they are never shown.
pub fn profile_of(tile: &Tile, draft: &ProfileDraft) -> Result<wire::Profile, Refused> {
    let shown = draft.shown;
    let display_name = if shown.name {
        line(&draft.display_name, MOST_NAME_CHARS, "Your name")?
    } else {
        None
    };
    let status = shown.status.then_some(match tile.status {
        TileStatus::Available => wire::ProfileStatus::Available,
        // Do not disturb shows to others as Busy (ADR-163 §8).
        TileStatus::Busy | TileStatus::DoNotDisturb => wire::ProfileStatus::Busy,
        TileStatus::Away => wire::ProfileStatus::Away,
    });
    let mood = match (&tile.mood, shown.mood) {
        (Some(word), true) => serde_json::from_value(serde_json::Value::String(word.clone())).ok(),
        _ => None,
    };
    let message = if shown.message {
        line(&tile.message, MOST_LINE_CHARS, "Your message")?
    } else {
        None
    };
    let company = if shown.company {
        line(&draft.company, MOST_LINE_CHARS, "Your company")?
    } else {
        None
    };
    let (business_kinds, business_line) = if shown.business {
        if draft.business_kinds.len() > MOST_KINDS {
            return Err(Refused("Choose at most 3 kinds of business.".into()));
        }
        let mut kinds: Vec<wire::BusinessKind> = Vec::new();
        for kind in &draft.business_kinds {
            let kind: wire::BusinessKind =
                serde_json::from_value(serde_json::Value::String(kind.clone()))
                    .ok()
                    .ok_or_else(|| Refused("That isn't one of the kinds of business.".into()))?;
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        (
            kinds,
            line(
                &draft.business_line,
                MOST_LINE_CHARS,
                "What your business does",
            )?,
        )
    } else {
        (Vec::new(), None)
    };
    let region = if shown.region && !draft.region.trim().is_empty() {
        let region = draft.region.trim();
        if !is_region(region) {
            return Err(Refused("Choose a state or a country.".into()));
        }
        Some(region.to_owned())
    } else {
        None
    };
    Ok(wire::Profile {
        display_name,
        status,
        mood,
        message,
        company,
        business_kinds,
        business_line,
        region,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile() -> Tile {
        Tile {
            status: TileStatus::DoNotDisturb,
            mood: Some("focused".into()),
            message: "Feeling great!".into(),
            picture: None,
        }
    }

    fn draft() -> ProfileDraft {
        ProfileDraft {
            display_name: "Pat Lee".into(),
            company: "Lee Builders".into(),
            business_kinds: vec!["construction".into(), "trades".into()],
            business_line: "Homes and repairs".into(),
            region: "US-CA".into(),
            shown: ProfileShown::default(),
        }
    }

    #[test]
    fn every_part_is_shown_to_begin_with_and_do_not_disturb_is_busy() {
        let profile = profile_of(&tile(), &draft()).unwrap();
        assert_eq!(profile.display_name.as_deref(), Some("Pat Lee"));
        assert_eq!(profile.status, Some(wire::ProfileStatus::Busy));
        assert_eq!(profile.mood, Some(wire::Mood::Focused));
        assert_eq!(profile.message.as_deref(), Some("Feeling great!"));
        assert_eq!(profile.company.as_deref(), Some("Lee Builders"));
        assert_eq!(
            profile.business_kinds,
            [wire::BusinessKind::Construction, wire::BusinessKind::Trades]
        );
        assert_eq!(profile.region.as_deref(), Some("US-CA"));
    }

    #[test]
    fn an_unticked_or_empty_part_is_never_sent() {
        let mut d = draft();
        d.shown = ProfileShown {
            picture: false,
            name: false,
            status: false,
            mood: false,
            message: false,
            company: false,
            business: false,
            region: false,
        };
        let profile = profile_of(&tile(), &d).unwrap();
        assert_eq!(
            profile,
            wire::Profile {
                display_name: None,
                status: None,
                mood: None,
                message: None,
                company: None,
                business_kinds: Vec::new(),
                business_line: None,
                region: None,
            }
        );
        let empty = ProfileDraft {
            shown: ProfileShown::default(),
            ..ProfileDraft::default()
        };
        let mut t = tile();
        t.message = "  ".into();
        t.mood = None;
        let profile = profile_of(&t, &empty).unwrap();
        assert_eq!(profile.display_name, None);
        assert_eq!(profile.message, None);
        assert_eq!(profile.mood, None);
        assert_eq!(profile.region, None);
        assert_eq!(profile.status, Some(wire::ProfileStatus::Busy));
    }

    #[test]
    fn what_cannot_be_on_a_card_is_refused_in_plain_words() {
        for (change, words) in [
            (
                Box::new(|d: &mut ProfileDraft| d.display_name = "a".repeat(61))
                    as Box<dyn Fn(&mut ProfileDraft)>,
                "at most 60",
            ),
            (Box::new(|d| d.company = "two\nlines".into()), "one line"),
            (
                Box::new(|d| d.region = "San Diego".into()),
                "state or a country",
            ),
            (Box::new(|d| d.region = "us".into()), "state or a country"),
            (
                Box::new(|d| {
                    d.business_kinds = vec!["a".into(), "b".into(), "c".into(), "d".into()]
                }),
                "at most 3",
            ),
            (
                Box::new(|d| d.business_kinds = vec!["spaceships".into()]),
                "kinds of business",
            ),
            (
                Box::new(|d| d.business_kinds = vec!["Construction".into()]),
                "kinds of business",
            ),
        ] {
            let mut d = draft();
            change(&mut d);
            let refused = profile_of(&tile(), &d).unwrap_err();
            assert!(refused.0.contains(words), "{}", refused.0);
        }
        // The same kind twice is kept once.
        let mut d = draft();
        d.business_kinds = vec!["construction".into(), "construction".into()];
        assert_eq!(profile_of(&tile(), &d).unwrap().business_kinds.len(), 1);
        assert!(is_region("US") && is_region("US-CA") && is_region("GB-ENG"));
        assert!(!is_region("USA") && !is_region("US-") && !is_region("U1"));
    }

    #[test]
    fn the_tile_is_read_as_it_is_kept() {
        assert_eq!(
            TileStatus::from_word("doNotDisturb"),
            TileStatus::DoNotDisturb
        );
        assert_eq!(TileStatus::from_word("away"), TileStatus::Away);
        assert_eq!(
            TileStatus::from_word("something new"),
            TileStatus::Available
        );
        assert_eq!(
            picture_from_base64("iVBORw==").unwrap(),
            [0x89, b'P', b'N', b'G']
        );
        assert_eq!(picture_from_base64("not base64!"), None);
        assert_eq!(picture_from_base64(&"A".repeat(400 * 1024)), None);
    }
}
