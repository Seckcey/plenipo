-- Plenipo Ledger, schema version 2: agent runtime sessions (Phase 3, ADR-007).
-- A runtime session is one conversation with an agent runtime (Claude Code, Codex, …). Its
-- turns are ordinary tasks whose metadata names the session (`$.sessionId`); each turn's
-- process is an ordinary execution.

CREATE TABLE runtime_sessions (
    id                         TEXT PRIMARY KEY,
    runtime                    TEXT NOT NULL CHECK (length(runtime) BETWEEN 1 AND 64),
    provider                   TEXT NOT NULL CHECK (length(provider) BETWEEN 1 AND 64),
    provider_session_id        TEXT CHECK (provider_session_id IS NULL
                                           OR length(provider_session_id) BETWEEN 1 AND 200),
    provider_session_confirmed INTEGER NOT NULL DEFAULT 0
                                   CHECK (provider_session_confirmed IN (0, 1)),
    model                      TEXT CHECK (model IS NULL OR length(model) BETWEEN 1 AND 128),
    title                      TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    working_dir                TEXT NOT NULL,
    state                      TEXT NOT NULL CHECK (state IN ('open', 'closed')),
    metadata                   TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at                 INTEGER NOT NULL,
    updated_at                 INTEGER NOT NULL,
    closed_at                  INTEGER
);
CREATE INDEX runtime_sessions_by_updated ON runtime_sessions (updated_at);

-- Turns of a session, in order.
CREATE INDEX tasks_by_session ON tasks (json_extract(metadata, '$.sessionId'), created_at);
