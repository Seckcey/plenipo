//! Plenipo Core — provider-neutral domain types and logic.
//!
//! Phase 0 scope: the shared DTOs that cross the Tauri command boundary.
//! Every DTO derives [`ts_rs::TS`] so the TypeScript definitions in
//! `packages/types` are generated from this crate and cannot drift silently.

pub mod dto;

pub use dto::{AppInfo, BuildProfile, CommandError, CommandErrorKind};

/// Human-facing product name.
pub const PRODUCT_NAME: &str = "Plenipo";
