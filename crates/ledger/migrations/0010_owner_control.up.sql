-- Plenipo Ledger, schema version 10: the owner's control over workers (Phase 17).
--
-- Specialties under each role (ADR-042); archive, bring back, and delete for good for agents,
-- departments, and projects, where an item deleted for good keeps a short record in its own row
-- so older history still names it (ADR-043); and the Workforce, the agents the owner saved to
-- hire again (ADR-045).

-- A specialty narrows a role: its own lines for the role's working instructions, and the models
-- and permissions it suggests (in `metadata`). Plenipo keeps the built-in ones up to date; the
-- owner's can be changed or removed. A removed specialty keeps its row, so positions that had it
-- still name it.
CREATE TABLE specialties (
    id         TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    role_id    TEXT NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    -- The title suggested for a new position with this specialty.
    title      TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 80),
    metadata   TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    removed_at INTEGER
);
CREATE UNIQUE INDEX specialties_unique_name ON specialties (role_id, lower(name))
    WHERE removed_at IS NULL;

ALTER TABLE positions ADD COLUMN specialty_id TEXT REFERENCES specialties (id);
-- Deleted for good: the row stays, archived, as a short record (its ID, title, role, and dates).
ALTER TABLE positions ADD COLUMN deleted_at INTEGER
    CHECK (deleted_at IS NULL OR archived_at IS NOT NULL);
ALTER TABLE departments ADD COLUMN archived_at INTEGER;
ALTER TABLE departments ADD COLUMN deleted_at INTEGER
    CHECK (deleted_at IS NULL OR archived_at IS NOT NULL);
ALTER TABLE projects ADD COLUMN archived_at INTEGER;
ALTER TABLE projects ADD COLUMN deleted_at INTEGER
    CHECK (deleted_at IS NULL OR archived_at IS NOT NULL);

-- An archived position changes in exactly two ways: it is brought back (active again), or it is
-- deleted for good (it stays archived and becomes a short record). Either way its ID, title,
-- role, and dates stay as they were. A short record never changes.
DROP TRIGGER positions_stay_archived;
CREATE TRIGGER positions_stay_archived BEFORE UPDATE ON positions
WHEN OLD.state = 'archived' AND NOT (
    NEW.id = OLD.id AND NEW.title = OLD.title AND NEW.role_id = OLD.role_id
    AND NEW.created_at = OLD.created_at
    AND (
        (OLD.deleted_at IS NULL AND NEW.deleted_at IS NULL AND NEW.state = 'active')
        OR (OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL AND NEW.state = 'archived'
            AND NEW.archived_at = OLD.archived_at)
    )
)
BEGIN
    SELECT RAISE(ABORT, 'an archived position cannot change');
END;

-- The Workforce: agents the owner saved to hire again. Each keeps its title, role, specialty,
-- AI settings and learning setting (`settings`), its experience (`experience`), and copies of the
-- lessons it wrote that the owner kept (`lessons`). The position it came from is now a short
-- record. Hiring it again, or deleting it for good, removes it from here; its events stay.
CREATE TABLE saved_agents (
    id            TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    title         TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 80),
    role_id       TEXT NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    specialty_id  TEXT REFERENCES specialties (id),
    from_position TEXT REFERENCES positions (id),
    settings      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(settings)),
    experience    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(experience)),
    lessons       TEXT NOT NULL DEFAULT '[]'
                  CHECK (json_valid(lessons) AND json_type(lessons) = 'array'),
    saved_at      INTEGER NOT NULL
);
CREATE INDEX saved_agents_by_role ON saved_agents (role_id, saved_at);
