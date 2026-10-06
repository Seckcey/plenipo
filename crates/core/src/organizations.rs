//! More than one organization (Phase 21, ADR-094): the list a window shows, how a new one
//! starts, and what deleting one for good would remove.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One of your organizations, as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgSummary {
    pub id: String,
    pub name: String,
    /// Your first organization: it keeps the PC's shared record (your Workforce, your tile), so
    /// it cannot be archived or deleted (ADR-094, Limits).
    pub first: bool,
    pub archived: bool,
    /// The window that asked shows it.
    pub here: bool,
    /// A window shows it (this one or another).
    pub in_window: bool,
    /// Programs running for it now (its AI tools' turns among them).
    pub working: u32,
    #[ts(type = "number")]
    pub created_at: u64,
}

/// A template a new organization can start from (none yet: "Templates are coming later").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Your organizations, and which one the window that asked shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgListing {
    /// The organization this window shows.
    pub current: String,
    /// Every organization, the first one first, then by when each was made.
    pub organizations: Vec<OrgSummary>,
    /// The templates a new organization can start from.
    pub templates: Vec<OrgTemplate>,
}

/// How a new organization starts (ADR-091 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum OrgStart {
    /// Plenipo's starting settings, as a new copy has.
    Scratch,
    /// A copy of another organization's setup: never its work or secrets.
    Copy { from: String },
    /// A template (none yet).
    Template { template: String },
}

/// Who syncs a folder online, when Plenipo can tell (ADR-205 §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FolderSync {
    /// OneDrive: Plenipo can see whether it is told to keep the folder on this device.
    OneDrive,
    /// Another sync service (Dropbox, Google Drive, iCloud): Plenipo can't see that.
    Other,
}

/// An organization's folder (ADR-205): where it is, or where a new one would go, and whether a
/// sync service keeps it online.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgFolderInfo {
    /// Where it is (or would be). `None`: this organization has no organization folder yet.
    pub path: Option<String>,
    /// The folder is there now.
    pub exists: bool,
    /// Who syncs it online, when Plenipo can tell.
    pub synced_by: Option<FolderSync>,
    /// For OneDrive: whether it is set to "Always keep on this device". `None`: Plenipo can't
    /// tell.
    pub kept_on_this_device: Option<bool>,
    /// Why this place can't be the organization's folder, in plain words.
    pub problem: Option<String>,
}

/// What opening an organization did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OrgOpened {
    /// It was already shown in a window, which came to the front.
    Focused,
    /// It opened in a new window.
    Opened,
    /// This window is loading it now.
    Switching,
}

/// A worker of an organization about to be deleted for good.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgWorker {
    pub position_id: String,
    pub title: String,
    pub role_name: String,
    pub tasks_done: u32,
    pub kept_lessons: u32,
}

/// What deleting an archived organization for good removes, and whom it can save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgDeletePreview {
    pub id: String,
    pub name: String,
    /// Workers with experience, who can be saved to your Workforce.
    pub experienced: Vec<OrgWorker>,
    /// Experienced workers whose role your first organization does not have.
    pub not_saved: Vec<OrgWorker>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_reads_as_the_page_sends_it() {
        let copy: OrgStart = serde_json::from_str(r#"{"kind":"copy","from":"abc"}"#).unwrap();
        assert_eq!(copy, OrgStart::Copy { from: "abc".into() });
        let scratch: OrgStart = serde_json::from_str(r#"{"kind":"scratch"}"#).unwrap();
        assert_eq!(scratch, OrgStart::Scratch);
        assert!(serde_json::from_str::<OrgStart>(r#"{"kind":"clone"}"#).is_err());
    }
}
