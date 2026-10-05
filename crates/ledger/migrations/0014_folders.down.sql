-- Reverses 0014_folders. Development/test use only (see ADR-006).
DROP TRIGGER folders_never_deleted;
DROP INDEX folders_one_each;
DROP TABLE folders;
