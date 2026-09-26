//! Plenipo Runtime — supervised local process execution.
//!
//! The [`Supervisor`] launches **approved launch profiles** only. A profile names an
//! executable that must pass the [`ExecutablePolicy`] allowlist, fixed arguments, a working
//! directory, and the only environment variables the child may receive beyond a small OS
//! baseline. Callers (the UI) select a profile by ID; they never supply paths or commands.
//!
//! Each launch gets a unique execution ID, streams batched stdout/stderr through an
//! [`EventSink`], can be cancelled or time out, and runs in its own process tree
//! (Windows Job Object with kill-on-close / Unix process group) so terminating it never
//! leaves descendants behind.

pub mod diagnostic;
pub mod dto;
pub mod error;
mod output;
pub mod policy;
pub mod profile;
pub mod store;
mod supervisor;

pub use dto::{
    ExecutionOutput, ExecutionRecord, ExecutionState, LaunchProfileInfo, LifecycleEvent,
    OutputBatch, OutputLine, OutputStream, RuntimeEvent, RuntimeOverview,
};
pub use error::RuntimeError;
pub use policy::{ExecutablePolicy, PolicyError};
pub use profile::{LaunchProfile, ProfileRegistry};
pub use store::{ExecutionStore, MetadataStore};
pub use supervisor::{EventSink, Supervisor, SupervisorConfig};

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
