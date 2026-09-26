use crate::policy::PolicyError;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("unknown launch profile: {0}")]
    UnknownProfile(String),
    #[error("unknown execution: {0}")]
    UnknownExecution(String),
    #[error("not allowed: {0}")]
    Policy(#[from] PolicyError),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("runtime is shutting down")]
    ShuttingDown,
    /// An agent runtime cannot take work right now (not installed, not signed in, busy, …).
    /// The message says what to do.
    #[error("{0}")]
    NotReady(String),
    #[error("unknown session: {0}")]
    UnknownSession(String),
    /// Persisting agent session state failed.
    #[error("could not record agent activity: {0}")]
    Store(String),
}

impl RuntimeError {
    /// True when the caller supplied something unacceptable (vs. an internal failure).
    pub fn is_caller_error(&self) -> bool {
        !matches!(self, Self::ShuttingDown | Self::Store(_))
    }
}
