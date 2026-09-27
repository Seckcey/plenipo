//! Agent runtimes (Phase 3, ADR-007): provider-neutral adapter contract, the Claude Code,
//! Codex, and Grok adapters, the shared ACP driver (ADR-015), CLI discovery, and the session
//! service that runs turns under the supervisor.

pub mod acp;
pub mod adapter;
pub mod claude_code;
pub mod codex;
pub mod discovery;
pub mod dto;
pub mod grok;
pub mod memory_store;
pub mod service;
pub mod tools;

pub use adapter::{ProviderSession, RuntimeAdapter, TurnParser, TurnRequest};
pub use discovery::HostEnv;
pub use dto::*;
pub use memory_store::MemorySessionStore;
pub use service::{
    unavailable_outcome, AgentConfig, AgentRuntime, AgentSink, SessionChange, SessionStart,
    SessionStore, StepNote, TurnDisposition, TurnEnd, TurnHook, TurnInput, TurnRef, TurnTask,
    MAX_PROMPT_BYTES, OWNER, STEP_SEQ,
};
pub use tools::{StepInfo, StepTools, TextFilter, ToolProvider, ToolServer};

/// The adapters this build ships, in display order.
pub fn builtin_adapters() -> Vec<std::sync::Arc<dyn RuntimeAdapter>> {
    vec![
        std::sync::Arc::new(claude_code::ClaudeCode),
        std::sync::Arc::new(codex::Codex),
        std::sync::Arc::new(grok::Grok),
    ]
}
