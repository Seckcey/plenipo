-- Plenipo Ledger, schema version 1.
-- Conventions: TEXT UUID ids; INTEGER Unix-epoch milliseconds; `metadata` JSON objects for
-- forward-compatible extension. Nothing here is ever hard-deleted except org entities with no
-- dependants (FKs restrict).

CREATE TABLE roles (
    id                    TEXT PRIMARY KEY,
    name                  TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 200),
    description           TEXT NOT NULL DEFAULT '',
    role_type             TEXT NOT NULL CHECK (role_type IN
                              ('superintendent', 'department_manager', 'project_coordinator', 'worker')),
    persistent            INTEGER NOT NULL CHECK (persistent IN (0, 1)),
    model_policy_id       TEXT,
    capability_profile_id TEXT,
    metadata              TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at            INTEGER NOT NULL
);

CREATE TABLE departments (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 200),
    description     TEXT NOT NULL DEFAULT '',
    manager_role_id TEXT REFERENCES roles (id) ON DELETE RESTRICT,
    status          TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    metadata        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at      INTEGER NOT NULL
);

CREATE TABLE projects (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 200),
    local_path     TEXT,
    repository_url TEXT,
    department_id  TEXT REFERENCES departments (id) ON DELETE RESTRICT,
    metadata       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at     INTEGER NOT NULL
);

CREATE TABLE agent_instances (
    id                  TEXT PRIMARY KEY,
    role_id             TEXT NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    runtime_provider    TEXT,
    provider_session_id TEXT,
    project_id          TEXT REFERENCES projects (id) ON DELETE RESTRICT,
    lifecycle_state     TEXT NOT NULL CHECK (lifecycle_state IN
                            ('starting', 'active', 'idle', 'retired', 'failed')),
    metadata            TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at          INTEGER NOT NULL,
    last_seen_at        INTEGER NOT NULL
);

CREATE TABLE tasks (
    id                  TEXT PRIMARY KEY,
    parent_task_id      TEXT REFERENCES tasks (id) ON DELETE RESTRICT,
    requested_by        TEXT NOT NULL CHECK (length(requested_by) BETWEEN 1 AND 200),
    assigned_to         TEXT,
    project_id          TEXT REFERENCES projects (id) ON DELETE RESTRICT,
    objective           TEXT NOT NULL CHECK (length(objective) BETWEEN 1 AND 10000),
    acceptance_criteria TEXT NOT NULL DEFAULT '',
    priority            INTEGER NOT NULL DEFAULT 2 CHECK (priority BETWEEN 0 AND 4),
    state               TEXT NOT NULL CHECK (state IN
                            ('queued', 'running', 'blocked', 'awaitingApproval',
                             'succeeded', 'failed', 'cancelled')),
    metadata            TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    started_at          INTEGER,
    completed_at        INTEGER
);
CREATE INDEX tasks_by_parent ON tasks (parent_task_id);
CREATE INDEX tasks_by_created ON tasks (created_at);

-- Append-only activity trail. `seq` gives one global, gap-tolerant, strictly increasing order.
CREATE TABLE events (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    id           TEXT NOT NULL UNIQUE,
    task_id      TEXT REFERENCES tasks (id) ON DELETE RESTRICT,
    execution_id TEXT,
    source       TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 200),
    destination  TEXT,
    event_type   TEXT NOT NULL CHECK (length(event_type) BETWEEN 3 AND 64),
    payload      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload)),
    created_at   INTEGER NOT NULL
);
CREATE INDEX events_by_task ON events (task_id, seq);
CREATE INDEX events_by_execution ON events (execution_id, seq);

CREATE TRIGGER events_are_append_only_update BEFORE UPDATE ON events
BEGIN
    SELECT RAISE(ABORT, 'events are append-only');
END;
CREATE TRIGGER events_are_append_only_delete BEFORE DELETE ON events
BEGIN
    SELECT RAISE(ABORT, 'events are append-only');
END;

CREATE TABLE executions (
    id             TEXT PRIMARY KEY,
    task_id        TEXT REFERENCES tasks (id) ON DELETE RESTRICT,
    runtime        TEXT NOT NULL,
    provider       TEXT,
    model          TEXT,
    session_id     TEXT,
    process_id     INTEGER,
    profile_id     TEXT,
    label          TEXT NOT NULL DEFAULT '',
    executable     TEXT,
    args           TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(args)),
    working_dir    TEXT,
    state          TEXT NOT NULL CHECK (state IN
                       ('starting', 'running', 'succeeded', 'failed', 'cancelled',
                        'timedOut', 'interrupted')),
    exit_code      INTEGER,
    detail         TEXT,
    started_at     INTEGER NOT NULL,
    ended_at       INTEGER,
    usage_metadata TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(usage_metadata))
);
CREATE INDEX executions_by_started ON executions (started_at);
CREATE INDEX executions_by_task ON executions (task_id);

CREATE TABLE approvals (
    id              TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL REFERENCES tasks (id) ON DELETE RESTRICT,
    action_type     TEXT NOT NULL CHECK (length(action_type) BETWEEN 1 AND 100),
    request_payload TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(request_payload)),
    state           TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'rejected', 'expired')),
    requested_at    INTEGER NOT NULL,
    expires_at      INTEGER,
    resolved_at     INTEGER,
    resolved_by     TEXT
);
CREATE INDEX approvals_by_task ON approvals (task_id);

CREATE TABLE artifacts (
    id            TEXT PRIMARY KEY,
    task_id       TEXT REFERENCES tasks (id) ON DELETE RESTRICT,
    artifact_type TEXT NOT NULL CHECK (length(artifact_type) BETWEEN 1 AND 100),
    local_path    TEXT,
    uri           TEXT,
    hash          TEXT,
    metadata      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at    INTEGER NOT NULL,
    CHECK (local_path IS NOT NULL OR uri IS NOT NULL)
);
CREATE INDEX artifacts_by_task ON artifacts (task_id);
