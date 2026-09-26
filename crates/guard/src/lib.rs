//! Plenipo Guard — permissions, the checks every worker action passes, and secret redaction
//! (Phase 7, ADR-013).
//!
//! The **capability registry** lists every permission a worker can hold (the rollout plan's
//! capabilities). **Permission sets** (the plan's capability profiles) give each a level:
//! allowed, ask the owner, or blocked. A role's set grants; a project's and a department's set
//! only narrow it. For each action a worker asks for, the **engine** checks the plan's layers in
//! order — role, project, department, the target (inside the project folder, not a blocked file
//! or command), the action's risk (the sensitive-action check), and the owner's explicit rules —
//! and explains its decision in one plain sentence. Nothing here carries the action out: the
//! capability broker (`plenipo-capabilities`) does, after asking Guard.

pub mod commands;
pub mod config;
pub mod defaults;
pub mod dto;
pub mod engine;
mod error;
pub mod paths;
pub mod redact;
pub mod registry;
pub mod sensitive;
mod service;

pub use commands::CommandLine;
pub use config::GuardConfig;
pub use dto::*;
pub use engine::{evaluate, level_for, levels_for, GrantState, LevelFor, Request, Scope};
pub use error::{GuardError, Result};
pub use paths::{PathRefusal, Resolved, Workspace};
pub use redact::Redactor;
pub use registry::Capability;
pub use service::{scope_in, Guard, OWNER, PLENIPO, SETTING};
