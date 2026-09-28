-- Reverses 0011_canvas_and_loans. Development/test use only (see ADR-006).
DROP TRIGGER loans_stay_ended;
DROP TRIGGER loans_never_deleted;
DROP INDEX loans_by_lead;
DROP INDEX loans_by_objective;
DROP INDEX loans_one_active;
DROP TABLE loans;
DROP TABLE canvas_places;
