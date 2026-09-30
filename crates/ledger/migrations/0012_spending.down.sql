-- Reverses 0012_spending. Development/test use only (see ADR-006).
DROP TRIGGER spending_stays_settled;
DROP TRIGGER spending_never_deleted;
DROP INDEX spending_by_task;
DROP INDEX spending_by_month;
DROP TABLE spending;
