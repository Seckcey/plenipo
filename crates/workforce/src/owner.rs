//! The owner's tile (Phase 18, ADR-056): a picture, a status, a mood, and a short message,
//! kept on this PC in the Ledger's `owner` setting. Nothing here leaves the PC (sharing a
//! profile is Phase 24). The picture arrives already shrunk by the window, as a PNG in base64,
//! and is checked before it is kept; Plenipo never opens a file for it.

use base64::Engine as _;
use plenipo_ledger::Ledger;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::error::{Result, WorkforceError};
use crate::service::OWNER;

/// The setting the profile is kept in.
pub const OWNER_SETTING: &str = "owner";
/// The longest message.
pub const MAX_MESSAGE_CHARS: usize = 80;
/// The largest picture, in pixels on each side and in bytes.
pub const MAX_PICTURE_SIDE: u32 = 256;
pub const MAX_PICTURE_BYTES: usize = 256 * 1024;

/// The owner's status light.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OwnerStatus {
    #[default]
    Available,
    Busy,
    Away,
    /// Windows pop-up notices wait while it is on; the bell still counts them.
    DoNotDisturb,
}

/// The owner's mood (a face and a word on screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Mood {
    Great,
    Good,
    Okay,
    Tired,
    Stressed,
    Focused,
    Celebrating,
}

/// The owner's tile, as the window shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OwnerProfile {
    pub status: OwnerStatus,
    pub mood: Option<Mood>,
    pub message: String,
    /// The picture: a PNG of at most 256 × 256, in base64; `None` shows the owner glyph.
    pub picture: Option<String>,
}

/// What to do with the picture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum PictureChange {
    Keep,
    /// A new picture: a PNG in base64, already shrunk by the window.
    Set {
        png: String,
    },
    Remove,
}

/// A change to the owner's tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OwnerProfileInput {
    pub status: OwnerStatus,
    pub mood: Option<Mood>,
    pub message: String,
    pub picture: PictureChange,
}

fn invalid(message: impl Into<String>) -> WorkforceError {
    WorkforceError::Invalid(message.into())
}

/// The message as it is kept: one line, trimmed, no control characters, at most 80 characters.
fn clean_message(message: &str) -> Result<String> {
    let message = message.trim();
    if message.chars().any(char::is_control) {
        return Err(invalid("your message must be one line of plain text"));
    }
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return Err(invalid(format!(
            "your message can be at most {MAX_MESSAGE_CHARS} characters"
        )));
    }
    Ok(message.to_owned())
}

/// Check a picture: base64 of a PNG that decodes, at most 256 × 256 pixels and 256 KB. Returns
/// it in the standard base64 form.
pub fn check_picture(encoded: &str) -> Result<String> {
    let refused = || invalid("the picture must be a PNG of at most 256 × 256 pixels and 256 KB");
    if encoded.len() > MAX_PICTURE_BYTES * 4 / 3 + 4 {
        return Err(refused());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_| refused())?;
    if bytes.len() > MAX_PICTURE_BYTES {
        return Err(refused());
    }
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes.as_slice()))
        .read_info()
        .map_err(|_| refused())?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > MAX_PICTURE_SIDE
        || info.height > MAX_PICTURE_SIDE
    {
        return Err(refused());
    }
    // Every pixel must decode: a broken picture is refused, not kept.
    let mut frame = vec![0; reader.output_buffer_size().ok_or_else(refused)?];
    reader.next_frame(&mut frame).map_err(|_| refused())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

fn read(value: &Value) -> OwnerProfile {
    serde_json::from_value(value.clone()).unwrap_or_default()
}

/// The owner's tile as it is now.
pub fn profile(ledger: &Ledger) -> Result<OwnerProfile> {
    Ok(ledger
        .setting(OWNER_SETTING)?
        .map(|v| read(&v))
        .unwrap_or_default())
}

/// Change the owner's tile, recorded as `owner.profile_changed` (which parts changed, the status,
/// and the mood — never the picture or the message's words).
pub fn set_profile(ledger: &Ledger, input: &OwnerProfileInput) -> Result<OwnerProfile> {
    let message = clean_message(&input.message)?;
    let picture = match &input.picture {
        PictureChange::Set { png } => Some(Some(check_picture(png)?)),
        PictureChange::Remove => Some(None),
        PictureChange::Keep => None,
    };
    let value =
        ledger.update_setting(OWNER_SETTING, "owner.profile_changed", OWNER, |current| {
            let before = read(&current);
            let after = OwnerProfile {
                status: input.status,
                mood: input.mood,
                message: message.clone(),
                picture: picture.clone().unwrap_or_else(|| before.picture.clone()),
            };
            let mut changed = Vec::new();
            if after.status != before.status {
                changed.push("status");
            }
            if after.mood != before.mood {
                changed.push("mood");
            }
            if after.message != before.message {
                changed.push("message");
            }
            if after.picture != before.picture {
                changed.push("picture");
            }
            let payload = json!({
                "changed": changed,
                "status": after.status,
                "mood": after.mood,
            });
            Ok((json!(after), payload))
        })?;
    Ok(read(&value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny PNG of `w` × `h` pixels, in base64.
    pub(crate) fn picture(w: u32, h: u32) -> String {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let data = vec![200u8; (w * h * 4) as usize];
            writer.write_image_data(&data).unwrap();
        }
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn input(status: OwnerStatus, picture: PictureChange) -> OwnerProfileInput {
        OwnerProfileInput {
            status,
            mood: Some(Mood::Great),
            message: "  Feeling great!  ".into(),
            picture,
        }
    }

    #[test]
    fn the_owners_picture_status_mood_and_message_are_saved_and_read_back() {
        let l = Ledger::open_in_memory().unwrap();
        assert_eq!(profile(&l).unwrap(), OwnerProfile::default());
        let pic = picture(64, 64);
        let saved = set_profile(
            &l,
            &input(OwnerStatus::Busy, PictureChange::Set { png: pic.clone() }),
        )
        .unwrap();
        assert_eq!(saved.status, OwnerStatus::Busy);
        assert_eq!(saved.mood, Some(Mood::Great));
        assert_eq!(saved.message, "Feeling great!");
        assert_eq!(saved.picture.as_deref(), Some(pic.as_str()));
        assert_eq!(profile(&l).unwrap(), saved);

        // Keep leaves the picture; Remove takes it away.
        let kept = set_profile(&l, &input(OwnerStatus::DoNotDisturb, PictureChange::Keep)).unwrap();
        assert_eq!(kept.picture.as_deref(), Some(pic.as_str()));
        let events = l.recent_events(10).unwrap();
        let last = &events[0];
        assert_eq!(last.event_type, "owner.profile_changed");
        assert_eq!(last.payload["changed"], json!(["status"]));
        assert_eq!(last.payload["status"], "doNotDisturb");
        assert!(
            events
                .iter()
                .all(|e| !e.payload.to_string().contains(&pic[..40])
                    && !e.payload.to_string().contains("Feeling")),
            "the picture and the message's words are never in an event"
        );
        let removed = set_profile(&l, &input(OwnerStatus::Away, PictureChange::Remove)).unwrap();
        assert_eq!(removed.picture, None);
    }

    #[test]
    fn only_a_small_real_picture_and_a_short_line_are_kept() {
        let l = Ledger::open_in_memory().unwrap();
        for bad in [
            "not base64!".to_owned(),
            base64::engine::general_purpose::STANDARD.encode(b"GIF89a not a png"),
            picture(257, 10),
            picture(10, 300),
        ] {
            let e = set_profile(
                &l,
                &input(OwnerStatus::Available, PictureChange::Set { png: bad }),
            )
            .unwrap_err()
            .to_string();
            assert!(e.contains("PNG of at most 256"), "{e}");
        }
        // A PNG cut short is refused, not kept.
        let whole = base64::engine::general_purpose::STANDARD
            .decode(picture(32, 32))
            .unwrap();
        let cut = base64::engine::general_purpose::STANDARD.encode(&whole[..whole.len() - 20]);
        assert!(set_profile(
            &l,
            &input(OwnerStatus::Available, PictureChange::Set { png: cut })
        )
        .is_err());
        let mut long = input(OwnerStatus::Available, PictureChange::Keep);
        long.message = "x".repeat(81);
        assert!(set_profile(&l, &long)
            .unwrap_err()
            .to_string()
            .contains("80"));
        let mut two = input(OwnerStatus::Available, PictureChange::Keep);
        two.message = "one\ntwo".into();
        assert!(set_profile(&l, &two)
            .unwrap_err()
            .to_string()
            .contains("one line"));
        assert_eq!(
            profile(&l).unwrap(),
            OwnerProfile::default(),
            "nothing kept"
        );
    }
}
