//! Plenipo Guard — permissions, the checks every worker action passes, and secret redaction
//! (Phase 7, ADR-013).
//!
//! The **capability registry** lists every permission a worker can hold (the rollout plan's
//! capabilities). **Permission sets** (the plan's capability profiles) give each a level:
//! allowed, ask the owner, or blocked. A role's set grants; a project's and a department's set
//! only narrow it. For each action a worker asks for, the **engine** checks the plan's layers in
//! order — role, project, department, the target (inside the project folder, not a blocked file
//! or command), the action's risk (the sensitive-action check), and the owner's explicit rules —
//! and explains its decision in one plain sentence. The owner's **website lists** (Phase 10) say
//! which websites workers may open in Plenipo's browser; the owner's **servers** (Phase 11) say
//! which servers workers may reach over SSH and what they may run there. Nothing here carries the
//! action out: the capability broker (`plenipo-capabilities`) does, after asking Guard.

pub mod add_ons;
pub mod ai_tools;
pub mod commands;
pub mod config;
pub mod connections;
pub mod defaults;
pub mod dto;
pub mod engine;
mod error;
pub mod outbound;
pub mod paid;
pub mod paths;
pub mod redact;
pub mod registry;
pub mod remote;
pub mod sensitive;
pub mod servers;
mod service;
pub mod websites;

pub use add_ons::{AddOn, AddOnChange, AddOnCheck, AddOnInput, AddOnTool, ToolMark};
pub use ai_tools::{AiToolAction, AiToolBusy, AiToolRequest};
pub use commands::CommandLine;
pub use config::GuardConfig;
pub use connections::{
    Access, AccessLevel, Account, AccountKind, Connection, ConnectionAction, ConnectionCheck,
    ConnectionRequest, ConnectionState, OwnApp, Part, PartLevel, Service, ToolKind, Who,
};
pub use dto::*;
pub use engine::{
    evaluate, level_for, level_for_add_on, level_for_connection, levels_for, GrantState, LevelFor,
    Request, Scope, SiteCheck,
};
pub use error::{GuardError, Result};
pub use outbound::{OutboundRules, Purpose};
pub use paid::{PaidKeyInfo, PaidProtocol, PaidService, MAX_PAID_KEYS};
pub use paths::{PathRefusal, Resolved, Workspace};
pub use redact::Redactor;
pub use registry::Capability;
pub use servers::{
    classify, Classified, CommandClass, Environment, HostKey, HostKeyInput, Server, ServerApproval,
    ServerCheck, ServerInput, ServerUse, SignIn,
};
pub use service::{scope_in, Guard, OWNER, PLENIPO, SETTING};
pub use websites::{OtherSites, Site, SiteVerdict, WebsiteRules};
