-- Agent custom tools (specs/061-agent-custom-tools, data-model.md).
--
-- One `custom_tool` row per tool, immutable `custom_tool_version` snapshots, agent-owned
-- `custom_tool_category` rows and the `custom_tool_event` log of actions that are not versions.
-- Times are epoch milliseconds. JSON is kept as text, read and written by the shared storage code.

CREATE TABLE IF NOT EXISTS custom_tool (
    id             TEXT COLLATE "C" PRIMARY KEY,
    agent_id       TEXT COLLATE "C" NOT NULL,
    name           TEXT COLLATE "C" NOT NULL,
    category       TEXT COLLATE "C" NOT NULL,
    current_seq    BIGINT NOT NULL,
    enabled        BIGINT NOT NULL DEFAULT 1,
    disabled_by    TEXT COLLATE "C",
    last_called_at BIGINT,
    created_at     BIGINT NOT NULL,
    updated_at     BIGINT NOT NULL,
    UNIQUE (agent_id, name)
);
CREATE INDEX IF NOT EXISTS idx_custom_tool_agent ON custom_tool(agent_id, enabled, category);

CREATE TABLE IF NOT EXISTS custom_tool_version (
    agent_id           TEXT COLLATE "C" NOT NULL,
    tool_id            TEXT COLLATE "C" NOT NULL,
    seq                BIGINT NOT NULL,
    name               TEXT COLLATE "C" NOT NULL,
    category           TEXT COLLATE "C" NOT NULL,
    description        TEXT COLLATE "C" NOT NULL,
    input_schema       TEXT COLLATE "C" NOT NULL,
    output_description TEXT COLLATE "C",
    script             TEXT COLLATE "C" NOT NULL,
    calls              TEXT COLLATE "C" NOT NULL,
    sample_input       TEXT COLLATE "C",
    actor_kind         TEXT COLLATE "C" NOT NULL,
    actor_id           TEXT COLLATE "C",
    actor_name         TEXT COLLATE "C",
    dream_cycle_id     TEXT COLLATE "C",
    trigger_kind       TEXT COLLATE "C" NOT NULL,
    restored_from      BIGINT,
    reason             TEXT COLLATE "C",
    created_at         BIGINT NOT NULL,
    PRIMARY KEY (tool_id, seq)
);
CREATE INDEX IF NOT EXISTS idx_custom_tool_version_agent ON custom_tool_version(agent_id);

CREATE TABLE IF NOT EXISTS custom_tool_category (
    agent_id    TEXT COLLATE "C" NOT NULL,
    name        TEXT COLLATE "C" NOT NULL,
    description TEXT COLLATE "C" NOT NULL,
    updated_by  TEXT COLLATE "C",
    updated_at  BIGINT NOT NULL,
    PRIMARY KEY (agent_id, name)
);

CREATE TABLE IF NOT EXISTS custom_tool_event (
    id             TEXT COLLATE "C" PRIMARY KEY,
    agent_id       TEXT COLLATE "C" NOT NULL,
    tool_id        TEXT COLLATE "C",
    tool_name      TEXT COLLATE "C",
    kind           TEXT COLLATE "C" NOT NULL,
    detail         TEXT COLLATE "C",
    actor_kind     TEXT COLLATE "C" NOT NULL,
    actor_id       TEXT COLLATE "C",
    actor_name     TEXT COLLATE "C",
    dream_cycle_id TEXT COLLATE "C",
    at             BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_custom_tool_event_agent ON custom_tool_event(agent_id, at DESC);
