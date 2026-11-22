-- Consultations (specs/064-consult-agents/data-model.md): a subagent row with a consultee is a
-- consultation of another agent, whose turns do the work.
ALTER TABLE subagent ADD COLUMN IF NOT EXISTS consultee_id TEXT COLLATE "C";
ALTER TABLE subagent ADD COLUMN IF NOT EXISTS principal_user_id TEXT COLLATE "C";
ALTER TABLE subagent ADD COLUMN IF NOT EXISTS follow_ups BIGINT NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_subagent_consultee ON subagent(consultee_id) WHERE consultee_id IS NOT NULL;
