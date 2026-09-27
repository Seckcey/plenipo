-- Plenipo Ledger, schema version 7: lessons workers learn from their work (ADR-024).
--
-- At the end of a task a worker may write down what would help the next worker in its role.
-- A lesson waits for the owner (Keep or Discard) unless the owner lets that role learn on its
-- own; lessons from tasks that used websites always wait. Kept lessons go into the
-- instructions of the role's later workers; the owner can remove one at any time.
CREATE TABLE lessons (
    id          TEXT PRIMARY KEY,
    role_id     TEXT NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    task_id     TEXT REFERENCES tasks (id) ON DELETE SET NULL,
    position_id TEXT,
    worker      TEXT NOT NULL DEFAULT '' CHECK (length(worker) <= 200),
    text        TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 400),
    state       TEXT NOT NULL CHECK (state IN ('waiting', 'kept', 'discarded', 'removed')),
    from_web    INTEGER NOT NULL DEFAULT 0 CHECK (from_web IN (0, 1)),
    created_at  INTEGER NOT NULL,
    decided_at  INTEGER,
    decided_by  TEXT CHECK (decided_by IS NULL OR length(decided_by) BETWEEN 1 AND 100)
);

CREATE INDEX lessons_by_role ON lessons (role_id, state, created_at);
CREATE INDEX lessons_by_state ON lessons (state, created_at);
