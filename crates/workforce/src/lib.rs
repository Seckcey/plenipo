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

pub mod directory;
pub mod dto;
pub mod error;
mod prompt;
mod service;
mod snapshot;
pub mod templates;
mod view;

pub use dto::*;
pub use error::{Result, WorkforceError};
pub use service::{Workforce, OWNER, PLENIPO};
