//! The workspace (Phase 21, ADR-092): the panels an organization's window shows, and the windows
//! it pops them out into.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A panel that can sit in a dock or in its own window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PanelId {
    /// The owner's terminals and the Watch tabs (ADR-031, ADR-055).
    Terminal,
    /// The file view (ADR-093).
    Files,
    /// Live conversations with the agents (ADR-200).
    Chat,
}

impl PanelId {
    pub const ALL: [PanelId; 3] = [PanelId::Terminal, PanelId::Files, PanelId::Chat];

    /// Its name in a window's label.
    pub fn key(self) -> &'static str {
        match self {
            PanelId::Terminal => "terminal",
            PanelId::Files => "files",
            PanelId::Chat => "chat",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    /// Its name on screen (a pop-out window's title).
    pub fn title(self) -> &'static str {
        match self {
            PanelId::Terminal => "Terminal",
            PanelId::Files => "Files",
            PanelId::Chat => "Chat",
        }
    }
}

/// What a pop-out window shows: a whole panel (ADR-092), or one agent's chat in a window of its
/// own (ADR-203), in one of [`PopOutTarget::CHAT_WINDOWS`] numbered windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
pub enum PopOutTarget {
    Panel { panel: PanelId },
    Chat { slot: u8 },
}

impl PopOutTarget {
    /// The most chats with windows of their own at once (the owner's choice, ADR-203).
    pub const CHAT_WINDOWS: u8 = 6;

    /// A chat's window is one of the numbered ones.
    pub fn is_valid(self) -> bool {
        match self {
            Self::Panel { .. } => true,
            Self::Chat { slot } => (1..=Self::CHAT_WINDOWS).contains(&slot),
        }
    }

    /// Its name in a window's label: the panel's (`terminal`), or `chat_3`. Labels allow only
    /// letters, digits, `-`, `/`, `:`, and `_`.
    pub fn key(self) -> String {
        match self {
            Self::Panel { panel } => panel.key().to_owned(),
            Self::Chat { slot } => format!("chat_{slot}"),
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        if let Some(panel) = PanelId::from_key(key) {
            return Some(Self::Panel { panel });
        }
        let digits = key.strip_prefix("chat_")?;
        if digits.len() != 1 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let target = Self::Chat {
            slot: digits.parse().ok()?,
        };
        target.is_valid().then_some(target)
    }

    /// Its name on screen (a pop-out window's first title; a chat's page names its agent).
    pub fn title(self) -> &'static str {
        match self {
            Self::Panel { panel } => panel.title(),
            Self::Chat { .. } => "Chat",
        }
    }
}

/// Where a window is on screen, in logical pixels (the screen's own scale taken out).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct WindowPlace {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WindowPlace {
    /// Every number is an ordinary one, and the size is one a window can have.
    pub fn is_sane(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 100_000.0)
            && self.width >= 1.0
            && self.height >= 1.0
    }
}

/// What happened to one of the window's pop-outs (the `plenipo://windows` event, to the
/// organization's window that owns it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum PopOutNotice {
    /// Its window closed (the owner closed it, or Plenipo did): the panel goes back to a dock, or
    /// the chat to the Chat panel.
    Closed { target: PopOutTarget },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_keys_round_trip() {
        for p in PanelId::ALL {
            assert_eq!(PanelId::from_key(p.key()), Some(p));
        }
        assert_eq!(PanelId::from_key("details"), None);
    }

    #[test]
    fn pop_out_keys_round_trip_and_chat_windows_are_numbered() {
        let mut all: Vec<PopOutTarget> = PanelId::ALL
            .into_iter()
            .map(|panel| PopOutTarget::Panel { panel })
            .collect();
        all.extend((1..=PopOutTarget::CHAT_WINDOWS).map(|slot| PopOutTarget::Chat { slot }));
        for t in all {
            assert!(t.is_valid());
            assert_eq!(PopOutTarget::from_key(&t.key()), Some(t), "{}", t.key());
        }
        assert_eq!(PopOutTarget::Chat { slot: 3 }.key(), "chat_3");
        for bad in [
            "chat_0", "chat_7", "chat_", "chat_12", "chat_-1", "chat_x", "details",
        ] {
            assert_eq!(PopOutTarget::from_key(bad), None, "{bad}");
        }
        assert!(!PopOutTarget::Chat { slot: 0 }.is_valid());
        assert!(!PopOutTarget::Chat { slot: 7 }.is_valid());
    }

    #[test]
    fn pop_out_targets_on_the_wire() {
        let panel = PopOutTarget::Panel {
            panel: PanelId::Files,
        };
        assert_eq!(
            serde_json::to_value(panel).unwrap(),
            serde_json::json!({ "kind": "panel", "panel": "files" })
        );
        let chat: PopOutTarget =
            serde_json::from_value(serde_json::json!({ "kind": "chat", "slot": 2 })).unwrap();
        assert_eq!(chat, PopOutTarget::Chat { slot: 2 });
        for bad in [
            serde_json::json!({ "kind": "chat", "slot": 2, "extra": 1 }),
            serde_json::json!({ "kind": "chat", "slot": 300 }),
            serde_json::json!({ "kind": "window", "slot": 1 }),
            serde_json::json!("terminal"),
        ] {
            assert!(
                serde_json::from_value::<PopOutTarget>(bad.clone()).is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            serde_json::to_value(PopOutNotice::Closed { target: chat }).unwrap(),
            serde_json::json!({ "kind": "closed", "target": { "kind": "chat", "slot": 2 } })
        );
    }

    #[test]
    fn only_ordinary_places_are_sane() {
        let ok = WindowPlace {
            x: -1200.0,
            y: 40.0,
            width: 640.0,
            height: 360.0,
        };
        assert!(ok.is_sane());
        assert!(!WindowPlace { width: 0.0, ..ok }.is_sane());
        assert!(!WindowPlace { x: f64::NAN, ..ok }.is_sane());
        assert!(!WindowPlace { y: 1e9, ..ok }.is_sane());
    }
}
