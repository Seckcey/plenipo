-- Plenipo Ledger, schema version 9: a lesson belongs to its project, and a lesson held for the
-- owner says why (ADR-050, lessons a role keeps on its own are notes, not orders).
--
-- project_id: the project of the task the lesson came from. Only workers on that project get
-- it; NULL means every worker of the role (a task outside any project, and every lesson kept
-- before this version).
-- held_reason: when a role that learns on its own must wait for the owner anyway (its task used
-- tools, or the lesson has a command, a path, or a web address), the reason in the owner's
-- words for the Learning page.
ALTER TABLE lessons ADD COLUMN project_id TEXT;
ALTER TABLE lessons ADD COLUMN held_reason TEXT
    CHECK (held_reason IS NULL OR length(held_reason) BETWEEN 1 AND 200);
