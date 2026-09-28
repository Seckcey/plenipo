//! Plenipo's own tools for a turn step (Phase 7, ADR-013). A [`ToolProvider`] (the capability
//! broker) may give a step a tool server: an MCP server over stdio that the AI tool starts
//! itself, whose every call Plenipo checks and carries out. The runtime only passes the server
//! to the adapter, opens it before the step's program starts, and closes it when the program
//! ends — before the step's result is recorded.
//!
//! An AI tool that cannot switch its own file tools off may instead ask Plenipo for each file
//! it reads or writes (ADR-027, Kimi over ACP): [`ToolProvider::file_access`] carries such a
//! request out through Guard, exactly as the worker's own call of Plenipo's tool would be.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
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
    /// The names of the tools the server offers this step (`read_file`, `run_command`, …), for
    /// AI tools that name a server's tool without the server (ADR-027).
    pub tools: Vec<String>,
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
    /// The AI tool the step runs on ("Claude Code").
    pub ai_tool: &'a str,
    /// Whether that AI tool can use Plenipo's tools at all (Ollama cannot).
    pub takes_tools: bool,
}

/// A file an AI tool asks Plenipo to read or write for it, instead of opening the file itself
/// (ADR-027). Paths are the AI tool's own: absolute, or relative to the project folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAccess {
    /// Read a text file: all of it, or `limit` lines from line `line` (counted from 1).
    Read {
        path: String,
        line: Option<usize>,
        limit: Option<usize>,
    },
    /// Create a file, or replace its whole content.
    Write { path: String, content: String },
}

/// Plenipo's answer to a [`FileAccess`]: the text read (for a write, a note of what was
/// done), or why it was not done.
pub type FileAnswer = Result<String, String>;

/// Work that finishes later (a call may wait for the owner's approval).
pub type Pending<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Gives turn steps Plenipo's tools (the capability broker).
pub trait ToolProvider: Send + Sync + 'static {
    /// A step is about to start: its tools, if it gets any. Called on a blocking thread.
    fn open(&self, step: &StepInfo<'_>) -> Option<StepTools>;
    /// A step that gets no tools: what its worker should know about that (for example, that it
    /// cannot open files or use websites, and why), placed before its prompt like the tools
    /// note. Called on a blocking thread, only when `open` gave nothing.
    fn note_without_tools(&self, _step: &StepInfo<'_>) -> Option<String> {
        None
    }
    /// The step's program ended (or never started): end its grant. Called on a blocking thread
    /// before the step's result is recorded.
    fn close(&self, grant_id: &str);
    /// Read or write a file for the AI tool of the step holding `grant_id`, under the worker's
    /// permissions (ADR-027): checked by Guard, recorded, and waiting for the owner when Guard
    /// says to ask. Default: refused.
    fn file_access(&self, grant_id: &str, access: FileAccess) -> Pending<FileAnswer> {
        let _ = (grant_id, access);
        Box::pin(async { Err("Plenipo cannot open files for this worker.".to_owned()) })
    }
    /// A change the AI tool of the step holding `grant_id` is still writing (Phase 18, ADR-055):
    /// shown in Watch only if the step's permissions let it change that file. Default: ignored.
    fn preview_write(&self, grant_id: &str, preview: crate::agent::preview::WritePreview) {
        let _ = (grant_id, preview);
    }
}

/// Hides secrets in text before it is shown or recorded (installed by the capability broker).
pub type TextFilter = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// `prompt` with the tools note before it.
pub fn with_note(note: &str, prompt: &str) -> String {
    format!("{NOTE_START}\n{}\n{NOTE_END}\n\n{prompt}", note.trim())
}
