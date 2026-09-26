//! Plenipo's own tools for a turn step (Phase 7, ADR-013). A [`ToolProvider`] (the capability
//! broker) may give a step a tool server: an MCP server over stdio that the AI tool starts
//! itself, whose every call Plenipo checks and carries out. The runtime only passes the server
//! to the adapter, opens it before the step's program starts, and closes it when the program
//! ends — before the step's result is recorded.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::agent::dto::AgentSession;

/// Name the AI tools know Plenipo's tool server by.
pub const SERVER_NAME: &str = "plenipo";

/// Marks the note about the tools placed before a step's prompt.
pub const NOTE_START: &str = "[Plenipo tools]";
pub const NOTE_END: &str = "[End of Plenipo tools]";

/// The tool server one step may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolServer {
    /// Name the AI tool knows it by ([`SERVER_NAME`]).
    pub name: String,
    /// Program the AI tool starts (Plenipo's own relay) and its arguments.
    pub command: PathBuf,
    pub args: Vec<String>,
    /// The same server in the common MCP configuration format (`{"mcpServers": …}`), for AI
    /// tools that read their servers from a file.
    pub config_file: PathBuf,
    /// Longest one tool call may take (an approval waits inside a call).
    pub call_timeout: Duration,
}

/// What the provider gives a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepTools {
    /// Identifies the grant, to close it when the step ends.
    pub grant_id: String,
    pub server: ToolServer,
    /// Told to the worker before its prompt (between [`NOTE_START`] and [`NOTE_END`]).
    pub note: String,
}

/// The step about to start.
#[derive(Debug, Clone, Copy)]
pub struct StepInfo<'a> {
    pub session: &'a AgentSession,
    pub task_id: &'a str,
    pub step: u32,
}

/// Gives turn steps Plenipo's tools (the capability broker).
pub trait ToolProvider: Send + Sync + 'static {
    /// A step is about to start: its tools, if it gets any. Called on a blocking thread.
    fn open(&self, step: &StepInfo<'_>) -> Option<StepTools>;
    /// The step's program ended (or never started): end its grant. Called on a blocking thread
    /// before the step's result is recorded.
    fn close(&self, grant_id: &str);
}

/// Hides secrets in text before it is shown or recorded (installed by the capability broker).
pub type TextFilter = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// `prompt` with the tools note before it.
pub fn with_note(note: &str, prompt: &str) -> String {
    format!("{NOTE_START}\n{}\n{NOTE_END}\n\n{prompt}", note.trim())
}
