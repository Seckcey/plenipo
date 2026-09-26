use plenipo_ledger::LedgerError;
use plenipo_router::RouterError;
use plenipo_runtime::RuntimeError;

#[derive(Debug, thiserror::Error)]
pub enum WorkforceError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Router(#[from] RouterError),
    /// The request is not acceptable; the message says why.
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Internal(String),
}

impl WorkforceError {
    /// True when the caller asked for something invalid (vs. an internal failure).
    pub fn is_caller_error(&self) -> bool {
        match self {
            Self::Ledger(e) => e.is_caller_error(),
            Self::Runtime(e) => e.is_caller_error(),
            Self::Router(e) => e.is_caller_error(),
            Self::Invalid(_) => true,
            Self::Internal(_) => false,
        }
    }
}

pub type Result<T, E = WorkforceError> = std::result::Result<T, E>;
