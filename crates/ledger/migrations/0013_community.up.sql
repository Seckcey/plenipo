-- Plenipo Ledger, schema version 13: Community's conversations and messages (Phase 24, ADR-164
-- §10, ADR-168 §3).
--
-- Kept on this PC, in the PC's shared record (the first organization's Ledger), in its backups and
-- exports, and never in Activity. One row for each person this PC talks with, or was asked by,
-- and one for each message or reaction. A message's words are here and nowhere else: never in an
-- event, never in a log. A message from someone else keeps its sealed proof (`payload`, `sig`,
-- `fk`, and the stamp), so you can report it (ADR-164 §6). **Delete for me** removes a message on
-- this PC only (ADR-172); **Delete my Community data from this PC** removes every row.
CREATE TABLE community_people (
    member_id    TEXT PRIMARY KEY CHECK (member_id GLOB 'cm_*' AND length(member_id) = 29),
    name         TEXT NOT NULL CHECK (length(name) BETWEEN 3 AND 30),
    display_name TEXT CHECK (display_name IS NULL OR length(display_name) <= 60),
    state        TEXT NOT NULL CHECK (state IN ('none', 'requestedByMe', 'requestedByThem',
                                                 'accepted', 'leftByMe', 'leftByThem')),
    safety_code  TEXT CHECK (safety_code IS NULL OR (length(safety_code) = 12
                                                     AND safety_code NOT GLOB '*[^0-9]*')),
    safety_seen  TEXT CHECK (safety_seen IS NULL OR (length(safety_seen) = 12
                                                     AND safety_seen NOT GLOB '*[^0-9]*')),
    -- You blocked them: this PC refuses their items too (ADR-167 §4).
    blocked      INTEGER NOT NULL DEFAULT 0 CHECK (blocked IN (0, 1)),
    updated_at   INTEGER NOT NULL
);

CREATE TABLE community_items (
    item_id     TEXT PRIMARY KEY CHECK (item_id GLOB 'ci_*' AND length(item_id) = 29),
    member_id   TEXT NOT NULL REFERENCES community_people (member_id) ON DELETE CASCADE,
    outgoing    INTEGER NOT NULL CHECK (outgoing IN (0, 1)),
    kind        TEXT NOT NULL CHECK (kind IN ('message', 'reaction')),
    body        TEXT NOT NULL CHECK (json_valid(body) AND length(body) <= 65536),
    sent_at     INTEGER NOT NULL,
    accepted_at INTEGER,
    request     INTEGER NOT NULL DEFAULT 0 CHECK (request IN (0, 1)),
    state       TEXT NOT NULL CHECK (state IN ('waiting', 'delivered', 'notDelivered',
                                               'received')),
    payload     TEXT CHECK (payload IS NULL OR length(payload) <= 65536),
    sig         TEXT CHECK (sig IS NULL OR length(sig) <= 128),
    fk          TEXT CHECK (fk IS NULL OR length(fk) <= 64),
    stamp       TEXT CHECK (stamp IS NULL OR length(stamp) <= 4096),
    seen        INTEGER NOT NULL DEFAULT 0 CHECK (seen IN (0, 1)),
    CHECK ((payload IS NULL) = (sig IS NULL) AND (sig IS NULL) = (fk IS NULL))
);
CREATE INDEX community_items_by_person ON community_items (member_id, sent_at);
