-- Plenipo Ledger, schema version 4: the Workforce (Phase 5, ADR-009).
-- Positions are the organization chart; agent instances fill them. A persistent position is
-- held by at most one active agent at a time (its incumbent) and may be vacant; an on-demand
-- position gets a new agent for every task delegated to it, bound to that task
-- (`agent_instances.task_id`), which leaves the active workforce when the task ends. Oversight
-- assigns a position to review, QA, or security-audit another position's team. Positions are
-- archived and oversight assignments ended; neither is ever deleted.

-- A role's type and persistence decide what its positions are, so they never change.
CREATE TRIGGER roles_class_is_fixed
BEFORE UPDATE OF role_type, persistent ON roles
WHEN OLD.role_type IS NOT NEW.role_type OR OLD.persistent IS NOT NEW.persistent
BEGIN
    SELECT RAISE(ABORT, 'a role''s type and persistence cannot change');
END;

CREATE TABLE positions (
    id          TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    title       TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 80),
    role_id     TEXT NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    -- The supervisor; NULL means the position reports to the owner.
    reports_to  TEXT REFERENCES positions (id) ON DELETE RESTRICT,
    -- The runtime (adapter ID) that fills the position, chosen by the owner.
    runtime_id  TEXT NOT NULL CHECK (length(runtime_id) BETWEEN 1 AND 32),
    model       TEXT CHECK (model IS NULL OR length(model) BETWEEN 1 AND 64),
    state       TEXT NOT NULL CHECK (state IN ('active', 'archived')),
    sort_key    INTEGER NOT NULL DEFAULT 0,
    metadata    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    archived_at INTEGER,
    CHECK (reports_to IS NULL OR reports_to != id),
    CHECK ((state = 'archived') = (archived_at IS NOT NULL))
);
CREATE INDEX positions_by_supervisor ON positions (reports_to, sort_key);
-- Titles are unique among a supervisor's active reports (ASCII case-insensitively here; the
-- full rule, which also covers a team's overseers, is checked with the write).
CREATE UNIQUE INDEX positions_unique_title ON positions (COALESCE(reports_to, ''), lower(title))
    WHERE state = 'active';

CREATE TRIGGER positions_role_is_fixed
BEFORE UPDATE OF role_id ON positions
WHEN OLD.role_id IS NOT NEW.role_id
BEGIN
    SELECT RAISE(ABORT, 'a position''s role cannot change');
END;
CREATE TRIGGER positions_stay_archived BEFORE UPDATE ON positions
WHEN OLD.state = 'archived'
BEGIN
    SELECT RAISE(ABORT, 'an archived position cannot change');
END;
CREATE TRIGGER positions_are_kept BEFORE DELETE ON positions
BEGIN
    SELECT RAISE(ABORT, 'positions are archived, never deleted');
END;

ALTER TABLE departments ADD COLUMN head_position_id TEXT REFERENCES positions (id);
CREATE UNIQUE INDEX departments_one_head ON departments (head_position_id)
    WHERE head_position_id IS NOT NULL;

ALTER TABLE projects ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN coordinator_position_id TEXT REFERENCES positions (id);
-- Runtime IDs the project's workers may use; empty allows none.
ALTER TABLE projects ADD COLUMN allowed_runtimes TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(allowed_runtimes) AND json_type(allowed_runtimes) = 'array');
-- Name of the default capability profile (granted by Guard from Phase 7; recorded only).
ALTER TABLE projects ADD COLUMN capability_profile TEXT
    CHECK (capability_profile IS NULL OR length(capability_profile) BETWEEN 1 AND 64);
ALTER TABLE projects ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'archived'));
CREATE UNIQUE INDEX projects_one_coordinator ON projects (coordinator_position_id)
    WHERE coordinator_position_id IS NOT NULL;

ALTER TABLE agent_instances ADD COLUMN position_id TEXT REFERENCES positions (id);
ALTER TABLE agent_instances ADD COLUMN runtime_id TEXT
    CHECK (runtime_id IS NULL OR length(runtime_id) BETWEEN 1 AND 32);
ALTER TABLE agent_instances ADD COLUMN model TEXT
    CHECK (model IS NULL OR length(model) BETWEEN 1 AND 64);
-- For a worker spawned by an on-demand position: the task it exists for.
ALTER TABLE agent_instances ADD COLUMN task_id TEXT REFERENCES tasks (id);
ALTER TABLE agent_instances ADD COLUMN retired_at INTEGER;
CREATE INDEX agents_by_position ON agent_instances (position_id, created_at);
CREATE UNIQUE INDEX agents_one_per_task ON agent_instances (task_id) WHERE task_id IS NOT NULL;
-- A persistent position has at most one incumbent.
CREATE UNIQUE INDEX agents_one_incumbent ON agent_instances (position_id)
    WHERE position_id IS NOT NULL AND task_id IS NULL
      AND lifecycle_state NOT IN ('retired', 'failed');

CREATE TABLE oversight (
    id          TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    kind        TEXT NOT NULL CHECK (kind IN ('review', 'qa', 'security')),
    overseer_id TEXT NOT NULL REFERENCES positions (id) ON DELETE RESTRICT,
    -- The lead of the team that is overseen.
    target_id   TEXT NOT NULL REFERENCES positions (id) ON DELETE RESTRICT,
    state       TEXT NOT NULL CHECK (state IN ('active', 'ended')),
    created_at  INTEGER NOT NULL,
    ended_at    INTEGER,
    CHECK (overseer_id != target_id),
    CHECK ((state = 'ended') = (ended_at IS NOT NULL))
);
CREATE UNIQUE INDEX oversight_once ON oversight (kind, overseer_id, target_id)
    WHERE state = 'active';
CREATE INDEX oversight_by_target ON oversight (target_id, state);
CREATE INDEX oversight_by_overseer ON oversight (overseer_id, state);

CREATE TRIGGER oversight_is_immutable
BEFORE UPDATE OF id, kind, overseer_id, target_id, created_at ON oversight
BEGIN
    SELECT RAISE(ABORT, 'oversight assignments are immutable');
END;
CREATE TRIGGER oversight_stays_ended BEFORE UPDATE ON oversight
WHEN OLD.state = 'ended'
BEGIN
    SELECT RAISE(ABORT, 'an ended oversight assignment cannot change');
END;
CREATE TRIGGER oversight_is_kept BEFORE DELETE ON oversight
BEGIN
    SELECT RAISE(ABORT, 'oversight assignments are never deleted');
END;

-- Small settings owned by Core (JSON values), e.g. the organization's name.
CREATE TABLE settings (
    key        TEXT PRIMARY KEY CHECK (length(key) BETWEEN 1 AND 64),
    value      TEXT NOT NULL CHECK (json_valid(value)),
    updated_at INTEGER NOT NULL
);

-- Work of a position (its tasks carry `metadata.workforce.positionId`), and the sessions of an
-- agent (`metadata.workforce.agentId`).
CREATE INDEX tasks_by_position
    ON tasks (json_extract(metadata, '$.workforce.positionId'), created_at);
CREATE INDEX sessions_by_agent
    ON runtime_sessions (json_extract(metadata, '$.workforce.agentId'), created_at);
