use plenipo_ledger::LedgerError;
use plenipo_runtime::RuntimeError;

#[derive(Debug, thiserror::Error)]
pub enum LiaisonError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("{0}")]
    Internal(String),
}

impl LiaisonError {
    /// True when the caller asked for something invalid (vs. an internal failure).
    pub fn is_caller_error(&self) -> bool {
        match self {
            Self::Ledger(e) => e.is_caller_error(),
            Self::Runtime(e) => e.is_caller_error(),
            Self::Internal(_) => false,
        }
    }
}

pub type Result<T, E = LiaisonError> = std::result::Result<T, E>;
