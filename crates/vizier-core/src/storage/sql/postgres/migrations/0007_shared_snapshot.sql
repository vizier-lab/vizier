-- Shared chat snapshots (specs/055-shared-chat data-model.md). The same shape as the SQLite
-- `init_share_schema`. `snapshot_json` is TEXT, never JSONB, so the stored bytes are the bytes
-- that were previewed.

CREATE TABLE shared_snapshot (
    share_id       TEXT COLLATE "C" PRIMARY KEY,
    creator_id     TEXT COLLATE "C" NOT NULL,
    agent_id       TEXT NOT NULL,
    topic_id       TEXT NOT NULL,
    title          TEXT NOT NULL,
    audience       TEXT COLLATE "C" NOT NULL CHECK (audience IN ('specific', 'signed_in', 'public')),
    include_trail  BIGINT NOT NULL,
    format_version BIGINT NOT NULL,
    size_bytes     BIGINT NOT NULL,
    created_at     BIGINT NOT NULL,
    revoked_at     BIGINT,
    revoked_by     TEXT,
    removals_json  TEXT NOT NULL,
    snapshot_json  TEXT NOT NULL
);
CREATE INDEX idx_shared_snapshot_creator ON shared_snapshot(creator_id, created_at DESC);
CREATE INDEX idx_shared_snapshot_created ON shared_snapshot(created_at DESC);

CREATE TABLE shared_snapshot_audience (
    share_id TEXT COLLATE "C" NOT NULL REFERENCES shared_snapshot(share_id) ON DELETE CASCADE,
    user_id  TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (share_id, user_id)
);
CREATE INDEX idx_shared_snapshot_audience_user ON shared_snapshot_audience(user_id);
