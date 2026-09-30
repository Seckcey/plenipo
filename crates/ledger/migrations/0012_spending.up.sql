-- Plenipo Ledger, schema version 12: spending on paid AI keys (Phase 16 Wave 3, ADR-085).
--
-- One row per paid task. Before the task's request is sent, the most it could cost is set aside
-- (`setAside`), and only if it fits under every spending cap that covers it. When the task ends,
-- the row says what it really cost (`spent`), that the bill could not be read (`notPriced`,
-- counted at the most it could have cost, never as zero), or that the request was never sent
-- (`released`, counted as nothing). A settled row never changes, and no row is ever deleted.
--
-- Positions and departments can be removed from the organization later, so their IDs are kept
-- as plain text with their names as they were, and past spending stays where it was spent. The
-- key is named by its reference ID and name only; the key itself is never stored here.
CREATE TABLE spending (
    id               TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    task_id          TEXT CHECK (task_id IS NULL OR length(task_id) BETWEEN 1 AND 64),
    execution_id     TEXT CHECK (execution_id IS NULL OR length(execution_id) BETWEEN 1 AND 64),
    position_id      TEXT CHECK (position_id IS NULL OR length(position_id) BETWEEN 1 AND 64),
    position_title   TEXT,
    department_id    TEXT CHECK (department_id IS NULL OR length(department_id) BETWEEN 1 AND 64),
    department_name  TEXT,
    runtime          TEXT NOT NULL CHECK (length(runtime) BETWEEN 1 AND 64),
    model            TEXT NOT NULL CHECK (length(model) BETWEEN 1 AND 200),
    key_id           TEXT CHECK (key_id IS NULL OR length(key_id) BETWEEN 1 AND 64),
    key_name         TEXT CHECK (key_name IS NULL OR length(key_name) BETWEEN 1 AND 100),
    month            TEXT NOT NULL CHECK (month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
    state            TEXT NOT NULL CHECK (state IN ('setAside', 'spent', 'notPriced', 'released')),
    set_aside_micros INTEGER NOT NULL CHECK (set_aside_micros >= 0),
    spent_micros     INTEGER CHECK (spent_micros IS NULL OR spent_micros >= 0),
    priced_by        TEXT CHECK (priced_by IS NULL OR priced_by IN ('service', 'priceList')),
    detail           TEXT CHECK (detail IS NULL OR length(detail) <= 500),
    created_at       INTEGER NOT NULL,
    settled_at       INTEGER,
    CHECK ((state = 'setAside') = (settled_at IS NULL)),
    CHECK ((state = 'spent') = (spent_micros IS NOT NULL)),
    CHECK ((state = 'spent') = (priced_by IS NOT NULL))
);
CREATE INDEX spending_by_month ON spending (month, state);
CREATE INDEX spending_by_task ON spending (task_id);

CREATE TRIGGER spending_never_deleted BEFORE DELETE ON spending
BEGIN
    SELECT RAISE(ABORT, 'spending records are never deleted');
END;

CREATE TRIGGER spending_stays_settled BEFORE UPDATE ON spending
WHEN OLD.state <> 'setAside'
BEGIN
    SELECT RAISE(ABORT, 'a settled spending record never changes');
END;
