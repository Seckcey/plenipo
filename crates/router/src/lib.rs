//! Plenipo Router — model policy and role routing (Phase 6, ADR-011).
//!
//! The owner keeps a **model registry** (the models each AI tool can run, under the owner's own
//! names, with what they can do and what they cost) and a **model policy per role** (preferred
//! models in order, requirements, AI companies never to use, cost and cross-company review
//! preferences). For every new worker — and for a full-time agent's new conversation — the
//! router picks the first model the policy allows whose AI tool is installed, signed in with a
//! subscription, allowed by the project, and not at a usage limit, and says why in plain words.
//! Nothing here names a vendor: AI tools and companies are data from the runtime adapters.

pub mod config;
pub mod dto;
pub mod engine;
mod error;
pub mod limits;
mod service;

pub use dto::*;
pub use engine::{route, RouteInput, ToolState};
pub use error::{Result, RouterError};
pub use service::{Planner, RouteRequest, Router, SETTING};
