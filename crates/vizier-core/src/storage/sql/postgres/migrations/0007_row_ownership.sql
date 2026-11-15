-- Row ownership and fencing (specs/057-fleet-row-ownership, data-model.md).
--
-- `owner_fence` holds one row per ownership scope (`agent:{id}` or `control`). Its `token` is
-- the fence: the database adds one on every claim, so it is monotonic across processes and
-- restarts, which the in-process lease counter is not. `lease_token` is the bus fencing token
-- a Fleet claim was made under (NULL for a single-node claim), so a holder whose lease expired
-- before it claimed can never overtake the one that replaced it.
--
-- `owner_node`/`owner_token` on the in-flight tables say which owner last claimed a row.
-- Recovery acts only on rows whose `owner_token` is older than the new fence; NULL is a row
-- written before this migration, owned by an expired owner (tracked in #82).
--
-- `0001_initial.sql` is never edited - an applied migration is skipped by the runner - so
-- the table and columns an existing deployment misses arrive here.

CREATE TABLE IF NOT EXISTS owner_fence (
    scope       TEXT COLLATE "C" PRIMARY KEY,
    node        TEXT COLLATE "C" NOT NULL,
    token       BIGINT NOT NULL,
    lease_token BIGINT,
    claimed_at  TEXT COLLATE "C" NOT NULL
);

ALTER TABLE session_turn ADD COLUMN IF NOT EXISTS owner_node TEXT COLLATE "C";
ALTER TABLE session_turn ADD COLUMN IF NOT EXISTS owner_token BIGINT;
ALTER TABLE task_run ADD COLUMN IF NOT EXISTS owner_node TEXT COLLATE "C";
ALTER TABLE task_run ADD COLUMN IF NOT EXISTS owner_token BIGINT;
ALTER TABLE background_job ADD COLUMN IF NOT EXISTS owner_node TEXT COLLATE "C";
ALTER TABLE background_job ADD COLUMN IF NOT EXISTS owner_token BIGINT;
ALTER TABLE client_tool_call ADD COLUMN IF NOT EXISTS owner_node TEXT COLLATE "C";
ALTER TABLE client_tool_call ADD COLUMN IF NOT EXISTS owner_token BIGINT;
ALTER TABLE sensitive_ask ADD COLUMN IF NOT EXISTS owner_node TEXT COLLATE "C";
ALTER TABLE sensitive_ask ADD COLUMN IF NOT EXISTS owner_token BIGINT;

-- The recovery predicates, per agent. `background_job (origin_agent, state)` is already
-- `idx_bg_job_agent`.
CREATE INDEX IF NOT EXISTS idx_client_call_agent_status ON client_tool_call (agent_id, status);
CREATE INDEX IF NOT EXISTS idx_sa_agent_state ON sensitive_ask (agent_id, state);
