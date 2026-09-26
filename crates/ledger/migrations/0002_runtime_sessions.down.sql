-- Reverses 0002_runtime_sessions. Development/test use only (see ADR-006).
DROP INDEX tasks_by_session;
DROP INDEX runtime_sessions_by_updated;
DROP TABLE runtime_sessions;
