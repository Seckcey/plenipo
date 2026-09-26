-- Plenipo Ledger, schema version 5: reasoning effort (Phase 6, ADR-011).
-- The effort level a runtime session runs every turn at (for example `high`); NULL means the
-- runtime's own default.
ALTER TABLE runtime_sessions ADD COLUMN effort TEXT
    CHECK (effort IS NULL OR length(effort) BETWEEN 1 AND 16);
