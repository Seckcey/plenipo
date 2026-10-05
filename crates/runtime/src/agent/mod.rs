//! Agent runtimes (Phase 3, ADR-007): provider-neutral adapter contract, the Claude Code,
//! Codex, Grok, Kimi, Ollama, Antigravity, and GitHub Copilot adapters, the shared ACP driver (ADR-015, ADR-027), CLI
//! discovery, and the session service that runs turns under the supervisor.

pub mod acp;
pub mod adapter;
pub mod antigravity;
pub mod brief;
pub mod claude_code;
pub mod codex;
pub mod copilot;
pub mod direct;
pub mod discovery;
pub mod dto;
pub mod grok;
pub mod kimi;
pub mod live_text;
pub mod memory_store;
pub mod ollama;
pub mod paid;
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
    unavailable_outcome, AgentConfig, AgentRuntime, AgentSink, Bridge, KeptActivity, NotFree,
    RuntimeHold, SessionChange, SessionStart, SessionStore, StepNote, TurnDisposition, TurnEnd,
    TurnHook, TurnInput, TurnRef, TurnTask, MAX_PROMPT_BYTES, OWNER, STEP_SEQ,
};
pub use tools::{
    FileAccess, FileAnswer, Pending, StepInfo, StepTools, TextFilter, ToolProvider, ToolServer,
};

/// The adapters this build ships, in display order.
pub fn builtin_adapters() -> Vec<std::sync::Arc<dyn RuntimeAdapter>> {
    let mut all: Vec<std::sync::Arc<dyn RuntimeAdapter>> = vec![
        std::sync::Arc::new(claude_code::ClaudeCode),
        std::sync::Arc::new(codex::Codex),
        std::sync::Arc::new(grok::Grok),
        std::sync::Arc::new(kimi::Kimi),
        std::sync::Arc::new(ollama::Ollama),
        std::sync::Arc::new(antigravity::Antigravity),
        std::sync::Arc::new(copilot::Copilot),
        std::sync::Arc::new(paid::OpenRouter),
    ];
    // Each AI company's own service with the owner's key (ADR-087).
    all.extend(direct::adapters());
    all
}
