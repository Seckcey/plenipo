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
}

impl RuntimeError {
    /// True when the caller supplied something unacceptable (vs. an internal failure).
    pub fn is_caller_error(&self) -> bool {
        !matches!(self, Self::ShuttingDown)
    }
}
