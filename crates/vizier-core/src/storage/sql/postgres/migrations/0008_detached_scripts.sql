-- Detached scripts replace background jobs (specs/058-detached-tool-calls, data-model.md).
--
-- One `detached_script` row per detached `execute_python` run. Its outcome lives in the row
-- (cut to 32 KiB), not in a blob; `delivered_at` is set by the one-transaction delivery claim.
-- Owned like the other in-flight tables (spec 057): `owner_node`/`owner_token`.
--
-- The `background_job`/`background_piece` tables and the `background_job` notifications go with
-- the feature they belonged to. There is no data migration: an old job has nothing to resume.
-- The drops and the delete are tracked for removal in #82.

CREATE TABLE IF NOT EXISTS detached_script (
    id              TEXT COLLATE "C" PRIMARY KEY,
    agent_id        TEXT COLLATE "C" NOT NULL,
    session_channel TEXT COLLATE "C" NOT NULL,
    session_topic   TEXT COLLATE "C",
    parent_id       TEXT COLLATE "C",
    depth           BIGINT NOT NULL,
    intent          TEXT COLLATE "C" NOT NULL,
    state           TEXT COLLATE "C" NOT NULL,
    timeout_secs    BIGINT NOT NULL,
    code            TEXT COLLATE "C" NOT NULL,
    tool_calls      TEXT COLLATE "C" NOT NULL DEFAULT '[]',
    created_at      BIGINT NOT NULL,
    finished_at     BIGINT,
    outcome         TEXT COLLATE "C",
    delivered_at    BIGINT,
    cancelled_by    TEXT COLLATE "C",
    owner_node      TEXT COLLATE "C",
    owner_token     BIGINT
);
CREATE INDEX IF NOT EXISTS idx_detached_agent_state ON detached_script(agent_id, state);
CREATE INDEX IF NOT EXISTS idx_detached_session ON detached_script(agent_id, session_channel, session_topic, state, delivered_at);
CREATE INDEX IF NOT EXISTS idx_detached_parent ON detached_script(parent_id);

DROP TABLE IF EXISTS background_piece;
DROP TABLE IF EXISTS background_job;
DELETE FROM notification WHERE category = 'background_job';
