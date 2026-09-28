-- Plenipo Ledger, schema version 11: the organization canvas (Phase 18).
--
-- Where the owner put each tile on the canvas (ADR-053), and agents lent to another team
-- (ADR-054).

-- A tile the owner placed by hand: the owner's tile ("owner"), the organization's
-- ("organization"), or a position's (its ID). A tile without a row is placed by the automatic
-- layout, next to its lead. Where things sit on the screen is not something that happened in
-- the organization, so placing a tile records no event (ADR-053 §4).
CREATE TABLE canvas_places (
    tile_id    TEXT PRIMARY KEY CHECK (length(tile_id) BETWEEN 1 AND 64),
    x          REAL NOT NULL CHECK (x BETWEEN -100000 AND 100000),
    y          REAL NOT NULL CHECK (y BETWEEN -100000 AND 100000),
    updated_at INTEGER NOT NULL
);

-- An on-call agent lent to another team: for one objective of that team (`objective`; the
-- objective it joins is `objective_task_id`, the root task), or until the owner sends it home
-- (`returned`). `going_home` is set when the owner sends it home while it works: it goes when
-- its task ends. The team's project and department are noted as they were when it was lent.
-- Rows are never removed, and an ended loan never changes.
CREATE TABLE loans (
    id                TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    position_id       TEXT NOT NULL REFERENCES positions (id),
    from_lead_id      TEXT REFERENCES positions (id),
    to_lead_id        TEXT NOT NULL REFERENCES positions (id),
    to_project_id     TEXT REFERENCES projects (id),
    to_department_id  TEXT REFERENCES departments (id),
    until             TEXT NOT NULL CHECK (until IN ('objective', 'returned')),
    objective_task_id TEXT REFERENCES tasks (id),
    state             TEXT NOT NULL CHECK (state IN ('active', 'ended')),
    going_home        INTEGER NOT NULL DEFAULT 0 CHECK (going_home IN (0, 1)),
    started_at        INTEGER NOT NULL,
    ended_at          INTEGER,
    end_reason        TEXT,
    metadata          TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    CHECK ((state = 'ended') = (ended_at IS NOT NULL)),
    CHECK (position_id <> to_lead_id)
);
CREATE UNIQUE INDEX loans_one_active ON loans (position_id) WHERE state = 'active';
CREATE INDEX loans_by_objective ON loans (objective_task_id) WHERE state = 'active';
CREATE INDEX loans_by_lead ON loans (to_lead_id) WHERE state = 'active';

CREATE TRIGGER loans_never_deleted BEFORE DELETE ON loans
BEGIN
    SELECT RAISE(ABORT, 'loans are never deleted');
END;

CREATE TRIGGER loans_stay_ended BEFORE UPDATE ON loans
WHEN OLD.state = 'ended'
BEGIN
    SELECT RAISE(ABORT, 'an ended loan cannot change');
END;
