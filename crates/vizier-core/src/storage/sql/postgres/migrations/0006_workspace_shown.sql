-- Spec 053 US7, revised: an agent's prose between tool rounds is no longer shown in a
-- Workspace: it stays in that agent's own trail. The rows are kept, because the timeline and
-- `workspace_read` order on `seq`, so the column says which entries a member sees and an agent
-- reads. Existing entries were all messages, so the default keeps them as they were.

ALTER TABLE workspace_entry ADD COLUMN shown BIGINT NOT NULL DEFAULT 1;