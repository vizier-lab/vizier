-- Interactive pages (specs/063-html-generative-ui, data-model.md).
--
-- One `ui_page` row per page an agent drew in a conversation and one `ui_page_version` row per
-- version. The HTML is in the blob store under `ui-pages/{agent}/{page}/{sha256}.html`. Times are
-- epoch milliseconds. JSON is kept as text, read and written by the shared storage code.

CREATE TABLE IF NOT EXISTS ui_page (
    id              TEXT COLLATE "C" PRIMARY KEY,
    agent_id        TEXT COLLATE "C" NOT NULL,
    session_channel TEXT COLLATE "C" NOT NULL,
    session_topic   TEXT COLLATE "C",
    title           TEXT NOT NULL,
    latest_version  BIGINT NOT NULL,
    state           TEXT,
    state_seq       BIGINT NOT NULL DEFAULT 0,
    created_at      BIGINT NOT NULL,
    updated_at      BIGINT NOT NULL,
    owner_node      TEXT COLLATE "C",
    owner_token     BIGINT
);
CREATE INDEX IF NOT EXISTS idx_ui_page_session ON ui_page(agent_id, session_channel, session_topic);

CREATE TABLE IF NOT EXISTS ui_page_version (
    page_id      TEXT COLLATE "C" NOT NULL,
    version      BIGINT NOT NULL,
    size         BIGINT NOT NULL,
    sha256       TEXT COLLATE "C" NOT NULL,
    origin       TEXT COLLATE "C" NOT NULL,
    base_version BIGINT,
    edit_count   BIGINT,
    verification TEXT NOT NULL,
    history_uid  TEXT COLLATE "C",
    created_at   BIGINT NOT NULL,
    owner_node   TEXT COLLATE "C",
    owner_token  BIGINT,
    PRIMARY KEY (page_id, version)
);
