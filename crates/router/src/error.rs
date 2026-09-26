use plenipo_ledger::LedgerError;

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    /// The request is not acceptable; the message says why.
    #[error("{0}")]
    Invalid(String),
}

impl RouterError {
    /// True when the caller asked for something invalid (vs. an internal failure).
    pub fn is_caller_error(&self) -> bool {
        match self {
            Self::Ledger(e) => e.is_caller_error(),
            Self::Invalid(_) => true,
        }
    }
}

pub type Result<T, E = RouterError> = std::result::Result<T, E>;
