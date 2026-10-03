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
    /// Its window closed (the owner closed it, or Plenipo did): the panel goes back to a dock.
    Closed { panel: PanelId },
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
