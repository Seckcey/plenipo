-- Reverses 0009_lessons_project_and_reason. Development/test use only (see ADR-006).
ALTER TABLE lessons DROP COLUMN held_reason;
ALTER TABLE lessons DROP COLUMN project_id;
