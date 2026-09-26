//! Plenipo capability broker — Plenipo's own tools for workers, runtime grants, the approval
//! queue, and the Vault (Phase 7, ADR-013).
//!
//! A worker of the organization with permissions gets Plenipo's tools through the standard
//! add-on channel its AI tool supports: an MCP server over stdio, which is Plenipo's own relay
//! (`--plenipo-tools=<ticket>`) talking to the running Plenipo on the loopback address. Every
//! call is checked by Plenipo Guard, carried out by Plenipo inside the project folder, recorded
//! in the Ledger, and answered with secrets hidden. A call Guard says to ask about waits for
//! the owner's approval.

pub mod broker;
pub mod dto;
mod error;
pub mod files;
pub mod mcp;
pub mod programs;
pub mod relay;
mod server;
pub mod tools;
pub mod vault;

pub use broker::{Broker, BrokerConfig, CallResult};
pub use dto::*;
pub use error::{BrokerError, Result};
pub use vault::{MemorySecretStore, OsSecretStore, SecretStore};
