//! Agent runtimes (Phase 3, ADR-007): provider-neutral adapter contract, the Claude Code,
//! Codex, Grok, Kimi, and Ollama adapters, the shared ACP driver (ADR-015, ADR-027), CLI
//! discovery, and the session service that runs turns under the supervisor.

pub mod acp;
pub mod adapter;
pub mod brief;
pub mod claude_code;
pub mod codex;
pub mod discovery;
pub mod dto;
pub mod grok;
pub mod kimi;
pub mod memory_store;
pub mod ollama;
pub mod preview;
pub mod service;
pub mod tools;

pub use adapter::{FileRequest, ProviderSession, RuntimeAdapter, TurnParser, TurnRequest};
pub use brief::{text_hash, BriefInput, LARGE_JOB_CHARS};
pub use discovery::HostEnv;
pub use dto::*;
pub use memory_store::MemorySessionStore;
pub use preview::{WritePreview, WriteTool};
pub use service::{
    unavailable_outcome, AgentConfig, AgentRuntime, AgentSink, Bridge, SessionChange, SessionStart,
    SessionStore, StepNote, TurnDisposition, TurnEnd, TurnHook, TurnInput, TurnRef, TurnTask,
    MAX_PROMPT_BYTES, OWNER, STEP_SEQ,
};
pub use tools::{
    FileAccess, FileAnswer, Pending, StepInfo, StepTools, TextFilter, ToolProvider, ToolServer,
};

/// The adapters this build ships, in display order.
pub fn builtin_adapters() -> Vec<std::sync::Arc<dyn RuntimeAdapter>> {
    vec![
        std::sync::Arc::new(claude_code::ClaudeCode),
        std::sync::Arc::new(codex::Codex),
        std::sync::Arc::new(grok::Grok),
        std::sync::Arc::new(kimi::Kimi),
        std::sync::Arc::new(ollama::Ollama),
    ]
}
