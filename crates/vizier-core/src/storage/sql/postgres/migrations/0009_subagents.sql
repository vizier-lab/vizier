-- Subagents as temporary workers (specs/059-subagent-workers, data-model.md).
--
-- One `subagent` row per worker. "Seen" and "delivered" are sequence numbers against
-- `outcome_seq`, so a worker that asks twice has two outcomes, each handed to the main agent once.
-- Owned like the other in-flight tables (spec 057): `owner_node`/`owner_token`.

CREATE TABLE IF NOT EXISTS subagent (
    id                 TEXT COLLATE "C" PRIMARY KEY,
    agent_id           TEXT COLLATE "C" NOT NULL,
    hiring_channel     TEXT COLLATE "C" NOT NULL,
    hiring_topic       TEXT COLLATE "C",
    parent_detached_id TEXT COLLATE "C",
    group_id           TEXT COLLATE "C" NOT NULL,
    position           BIGINT NOT NULL,
    group_size         BIGINT NOT NULL,
    worker_topic       TEXT COLLATE "C" NOT NULL,
    task               TEXT COLLATE "C" NOT NULL,
    timeout_secs       BIGINT NOT NULL,
    state              TEXT COLLATE "C" NOT NULL,
    stop_reason        TEXT COLLATE "C",
    question           TEXT COLLATE "C",
    result             TEXT COLLATE "C",
    result_truncated   BIGINT NOT NULL DEFAULT 0,
    error              TEXT COLLATE "C",
    outcome_seq        BIGINT NOT NULL DEFAULT 0,
    seen_seq           BIGINT NOT NULL DEFAULT 0,
    delivered_seq      BIGINT NOT NULL DEFAULT 0,
    deadline_at        BIGINT,
    remaining_ms       BIGINT,
    created_at         BIGINT NOT NULL,
    updated_at         BIGINT NOT NULL,
    ended_at           BIGINT,
    owner_node         TEXT COLLATE "C",
    owner_token        BIGINT
);
CREATE INDEX IF NOT EXISTS idx_subagent_hiring ON subagent(agent_id, hiring_channel, hiring_topic, created_at);
CREATE INDEX IF NOT EXISTS idx_subagent_agent_state ON subagent(agent_id, state);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subagent_worker ON subagent(worker_topic);
