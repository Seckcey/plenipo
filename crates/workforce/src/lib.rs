//! Plenipo Workforce — the organization engine (Phase 5, ADR-009).
//!
//! The company is a tree of positions: persistent positions (superintendents, department
//! managers, project coordinators) are held by one agent at a time, which keeps its
//! conversation; on-demand positions (developers, reviewers, QA engineers, …) get a new,
//! ephemeral worker for every task delegated to them. Departments and projects hang off their
//! heads and coordinators, and oversight assignments put reviewers, QA evaluators, and security
//! auditors on a team. The Ledger enforces the structure; this crate builds the snapshot the
//! canvas shows, applies the owner's changes, gives persistent agents their objectives, and
//! tells Liaison who each member's team is.

mod conversation;
pub mod directory;
pub mod dto;
pub mod error;
pub mod learning;
pub mod outcome;
pub mod owner;
mod prompt;
mod service;
pub mod side_chat;
mod snapshot;
pub mod templates;
mod view;

pub use dto::*;
pub use error::{Result, WorkforceError};
pub use learning::{LearningSettings, LearningSnapshot};
pub use outcome::ObjectiveReport;
pub use owner::{Mood, OwnerProfile, OwnerProfileInput, OwnerStatus, PictureChange};
pub use service::{reset_page, EntitlementsCell, Workforce, OWNER, PLENIPO};
