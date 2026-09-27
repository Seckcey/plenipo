-- Plenipo Ledger, schema version 8: activity over time (Phase 12A, ADR-030).
--
-- Activity strips count events in fixed time buckets for a department, project, or position.
-- This index keeps that fast on a large Ledger. No table changes.
CREATE INDEX events_by_created ON events (created_at);
