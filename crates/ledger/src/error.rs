#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid {entity} transition for {id}: {from} -> {to}")]
    InvalidTransition {
        entity: &'static str,
        id: String,
        from: String,
        to: String,
    },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error(
        "the ledger database uses schema version {db}, which is newer than this version of \
         Plenipo supports (version {app}); it was not opened, to protect it"
    )]
    NewerSchema { db: u32, app: u32 },
    #[error("migration {0} was modified after it was applied")]
    ModifiedMigration(u32),
    #[error("migration history is inconsistent: {0}")]
    InconsistentMigrations(String),
}

impl LedgerError {
    /// True when the caller asked for something invalid (vs. an internal/storage failure).
    pub fn is_caller_error(&self) -> bool {
        matches!(
            self,
            Self::NotFound(_) | Self::InvalidTransition { .. } | Self::InvalidInput(_)
        )
    }
}

pub type Result<T, E = LedgerError> = std::result::Result<T, E>;
