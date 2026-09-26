-- Reverses 0001_initial. Development/test use only: production rolls back by restoring the
-- automatic pre-migration backup (see ADR-006).
DROP TABLE artifacts;
DROP TABLE approvals;
DROP TABLE executions;
DROP TRIGGER events_are_append_only_delete;
DROP TRIGGER events_are_append_only_update;
DROP TABLE events;
DROP TABLE tasks;
DROP TABLE agent_instances;
DROP TABLE projects;
DROP TABLE departments;
DROP TABLE roles;
