-- Plenipo Ledger, schema version 14: the organization folder (Phase 25, ADR-205).
--
-- Where the organization's own folders are on this PC: the organization folder, and inside it a
-- folder for each department and project, their Files and Scratch pads folders, and each
-- position's scratch pad. Plenipo makes each one (or adopts a plain folder the owner already
-- has at that place) and records it here, once. A recorded folder is never moved or renamed by
-- Plenipo on its own, so its path stays when a name changes; one that has gone missing is made
-- again at the same path. Rows are never deleted: a folder whose department, project, or
-- position is gone is still the owner's, with the files in it.
CREATE TABLE folders (
    id              TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    kind            TEXT NOT NULL CHECK (kind IN ('organization', 'department',
                                                  'department_files', 'project', 'project_files',
                                                  'scratch_pads', 'scratch_pad')),
    -- What it belongs to: a department's, a project's, or a position's ID. The organization
    -- folder, and the organization's own Scratch pads folder, belong to the organization (NULL).
    ref_id          TEXT CHECK (ref_id IS NULL OR length(ref_id) BETWEEN 1 AND 64),
    path            TEXT NOT NULL UNIQUE CHECK (length(path) BETWEEN 1 AND 1000),
    -- Made by Plenipo (1), or a plain folder the owner already had at that place (0).
    made_by_plenipo INTEGER NOT NULL CHECK (made_by_plenipo IN (0, 1)),
    created_at      INTEGER NOT NULL,
    CHECK ((kind = 'organization') <= (ref_id IS NULL)),
    CHECK (kind IN ('organization', 'scratch_pads') OR ref_id IS NOT NULL)
);
CREATE UNIQUE INDEX folders_one_each ON folders (kind, COALESCE(ref_id, ''));

CREATE TRIGGER folders_never_deleted BEFORE DELETE ON folders
BEGIN
    SELECT RAISE(ABORT, 'folders are never deleted');
END;
