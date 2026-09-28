-- Reverses 0010_owner_control. Development/test use only (see ADR-006).
DROP INDEX saved_agents_by_role;
DROP TABLE saved_agents;
DROP TRIGGER positions_stay_archived;
CREATE TRIGGER positions_stay_archived BEFORE UPDATE ON positions
WHEN OLD.state = 'archived'
BEGIN
    SELECT RAISE(ABORT, 'an archived position cannot change');
END;
ALTER TABLE projects DROP COLUMN deleted_at;
ALTER TABLE projects DROP COLUMN archived_at;
ALTER TABLE departments DROP COLUMN deleted_at;
ALTER TABLE departments DROP COLUMN archived_at;
ALTER TABLE positions DROP COLUMN deleted_at;
ALTER TABLE positions DROP COLUMN specialty_id;
DROP INDEX specialties_unique_name;
DROP TABLE specialties;
