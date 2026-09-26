//! Agent runtimes (Phase 3, ADR-007): provider-neutral adapter contract, the Claude Code and
//! Codex adapters, CLI discovery, and the session service that runs turns under the
//! supervisor.

pub mod adapter;
pub mod claude_code;
pub mod codex;
pub mod discovery;
pub mod dto;
pub mod memory_store;
pub mod service;

pub use adapter::{ProviderSession, RuntimeAdapter, TurnParser, TurnRequest};
pub use discovery::HostEnv;
pub use dto::*;
pub use memory_store::MemorySessionStore;
pub use service::{AgentConfig, AgentRuntime, AgentSink, SessionChange, SessionStore};

/// The adapters this build ships, in display order.
pub fn builtin_adapters() -> Vec<std::sync::Arc<dyn RuntimeAdapter>> {
    vec![
        std::sync::Arc::new(claude_code::ClaudeCode),
        std::sync::Arc::new(codex::Codex),
    ]
}
