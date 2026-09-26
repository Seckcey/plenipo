-- Plenipo Ledger, schema version 3: Liaison messages (Phase 4, ADR-008).
-- A handoff request asks for a child task on another worker; its reply carries the child's
-- normalized result back to the requesting task. Every change to a message writes its
-- `liaison.*` events on the affected tasks in the same transaction. Messages are never
-- deleted, and only their state (and `updated_at`) ever changes.

CREATE TABLE liaison_messages (
    id              TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    correlation_id  TEXT NOT NULL CHECK (length(correlation_id) BETWEEN 1 AND 64),
    kind            TEXT NOT NULL CHECK (kind IN ('request', 'reply')),
    in_reply_to     TEXT REFERENCES liaison_messages (id) ON DELETE RESTRICT,
    -- The requesting task (for a reply: the task the reply is addressed to).
    task_id         TEXT NOT NULL REFERENCES tasks (id) ON DELETE RESTRICT,
    -- The child task created for a request (for a reply: the child that answered, if any).
    child_task_id   TEXT REFERENCES tasks (id) ON DELETE RESTRICT,
    source          TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 200),
    destination     TEXT NOT NULL CHECK (length(destination) BETWEEN 1 AND 200),
    state           TEXT NOT NULL,
    dedupe_key      TEXT NOT NULL UNIQUE CHECK (length(dedupe_key) BETWEEN 1 AND 200),
    envelope        TEXT NOT NULL CHECK (json_valid(envelope)),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    CHECK ((kind = 'reply') = (in_reply_to IS NOT NULL)),
    CHECK (kind != 'request' OR state IN ('accepted', 'dispatched', 'answered', 'cancelled', 'rejected')),
    CHECK (kind != 'reply' OR state IN ('pending', 'delivered', 'discarded'))
);
-- A request is answered at most once, and a child task belongs to one request.
CREATE UNIQUE INDEX liaison_one_reply_per_request ON liaison_messages (in_reply_to)
    WHERE in_reply_to IS NOT NULL;
CREATE UNIQUE INDEX liaison_one_request_per_child ON liaison_messages (child_task_id)
    WHERE kind = 'request' AND child_task_id IS NOT NULL;
CREATE INDEX liaison_messages_by_task ON liaison_messages (task_id, created_at);
CREATE INDEX liaison_messages_by_correlation ON liaison_messages (correlation_id, created_at);
CREATE INDEX liaison_messages_by_state ON liaison_messages (state);

CREATE TRIGGER liaison_messages_are_immutable
BEFORE UPDATE OF id, correlation_id, kind, in_reply_to, task_id, child_task_id, source,
                 destination, dedupe_key, envelope, created_at ON liaison_messages
BEGIN
    SELECT RAISE(ABORT, 'liaison messages are immutable');
END;
CREATE TRIGGER liaison_messages_are_kept BEFORE DELETE ON liaison_messages
BEGIN
    SELECT RAISE(ABORT, 'liaison messages are never deleted');
END;
