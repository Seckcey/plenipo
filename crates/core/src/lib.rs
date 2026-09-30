//! Plenipo Core — provider-neutral domain types and logic.
//!
//! Phase 0 scope: the shared DTOs that cross the Tauri command boundary.
//! Every DTO derives [`ts_rs::TS`] so the TypeScript definitions in
//! `packages/types` are generated from this crate and cannot drift silently.

pub mod dto;
pub mod organizations;
pub mod upkeep;
pub mod workspace;

pub use dto::{
    AppInfo, BuildProfile, CommandError, CommandErrorKind, LocalPath, SyntheticTaskAction,
};
pub use organizations::{
    OrgDeletePreview, OrgListing, OrgOpened, OrgStart, OrgSummary, OrgTemplate, OrgWorker,
};
pub use upkeep::{
    AvailableUpdate, CloseWindow, DiagnosticsFile, Recovery, RecoveryCause, RecoveryStatus,
    SettingsProblem, StartAndClose, StartAndCloseInput, StoppedTask, UpdateState, UpdateStatus,
    WindowRecovery,
};
pub use workspace::{PanelId, PopOutNotice, WindowPlace};

/// Human-facing product name.
pub const PRODUCT_NAME: &str = "Plenipo";
