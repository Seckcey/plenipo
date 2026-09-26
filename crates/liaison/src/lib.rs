//! Plenipo Liaison — the message bus through which agent workers hand tasks to one another
//! (Phase 4, ADR-008).
//!
//! Workers never control each other. A worker in a session that allows handoffs asks for help
//! in its answer ([`protocol`]); Liaison checks the request, records it with a child task in
//! the Ledger, starts a new worker for it with a small context packet ([`context`]), returns
//! the child's result as a reply, and resumes the requester once all its replies are in — all
//! recorded under one correlation ID per workflow.

pub mod address;
pub mod context;
pub mod dto;
pub mod error;
pub mod protocol;
mod service;
pub mod store;

pub use dto::*;
pub use error::{LiaisonError, Result};
pub use service::{Liaison, LiaisonConfig, ACTOR};
