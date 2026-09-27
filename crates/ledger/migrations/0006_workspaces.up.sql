-- Plenipo Ledger, schema version 6: working copies (Phase 8, ADR-016).
--
-- A project whose folder is a git repository gives each objective its own branch and working
-- copy (a git worktree), unless the owner turns it off.
ALTER TABLE projects ADD COLUMN branch_per_objective INTEGER NOT NULL DEFAULT 1
    CHECK (branch_per_objective IN (0, 1));

-- One working copy per objective (its workflow) and project. A worker that changes files while
-- another one of the same objective holds that working copy gets its own, made from it
-- (`parent_id`). `facts` is the branch as last seen: its commits and changed files.
CREATE TABLE workspaces (
    id             TEXT PRIMARY KEY,
    project_id     TEXT NOT NULL REFERENCES projects (id) ON DELETE RESTRICT,
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) BETWEEN 1 AND 64),
    root_task_id   TEXT REFERENCES tasks (id) ON DELETE RESTRICT,
    parent_id      TEXT REFERENCES workspaces (id) ON DELETE RESTRICT,
    repository     TEXT NOT NULL CHECK (length(repository) BETWEEN 1 AND 1024),
    subfolder      TEXT NOT NULL DEFAULT '' CHECK (length(subfolder) <= 1024),
    path           TEXT NOT NULL UNIQUE CHECK (length(path) BETWEEN 1 AND 1024),
    branch         TEXT NOT NULL CHECK (length(branch) BETWEEN 1 AND 200),
    base_ref       TEXT CHECK (base_ref IS NULL OR length(base_ref) BETWEEN 1 AND 200),
    base_commit    TEXT NOT NULL CHECK (length(base_commit) BETWEEN 4 AND 64),
    state          TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'removed')),
    facts          TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(facts)),
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    removed_at     INTEGER
);

CREATE INDEX workspaces_by_workflow ON workspaces (correlation_id, created_at);
CREATE INDEX workspaces_by_project ON workspaces (project_id, created_at);
CREATE UNIQUE INDEX workspaces_one_per_objective ON workspaces (project_id, correlation_id)
    WHERE parent_id IS NULL;
