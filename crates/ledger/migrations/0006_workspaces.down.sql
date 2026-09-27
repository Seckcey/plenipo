-- Reverses 0006_workspaces. Development/test use only (see ADR-006).
DROP TABLE workspaces;
ALTER TABLE projects DROP COLUMN branch_per_objective;
