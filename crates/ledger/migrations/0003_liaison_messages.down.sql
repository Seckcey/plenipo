-- Reverses 0003_liaison_messages. Development/test use only (see ADR-006).
DROP TRIGGER liaison_messages_are_kept;
DROP TRIGGER liaison_messages_are_immutable;
DROP INDEX liaison_messages_by_state;
DROP INDEX liaison_messages_by_correlation;
DROP INDEX liaison_messages_by_task;
DROP INDEX liaison_one_request_per_child;
DROP INDEX liaison_one_reply_per_request;
DROP TABLE liaison_messages;
