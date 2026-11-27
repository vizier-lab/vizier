-- Platform account linking (specs/069-platform-account-linking data-model.md). The same shape as
-- the SQLite `init_platform_link_schema`.

CREATE TABLE platform_link (
    platform              TEXT COLLATE "C" NOT NULL,
    platform_user_id      TEXT COLLATE "C" NOT NULL,
    user_id               TEXT COLLATE "C" NOT NULL REFERENCES "user"(user_id) ON DELETE CASCADE,
    platform_username     TEXT,
    platform_display_name TEXT,
    linked_at             TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (platform, platform_user_id),
    UNIQUE (user_id, platform)
);

CREATE TABLE platform_connect_code (
    platform              TEXT COLLATE "C" NOT NULL,
    platform_user_id      TEXT COLLATE "C" NOT NULL,
    code_hash             TEXT COLLATE "C" NOT NULL UNIQUE,
    platform_username     TEXT,
    platform_display_name TEXT,
    created_at_ms         BIGINT NOT NULL,
    expires_at_ms         BIGINT NOT NULL,
    PRIMARY KEY (platform, platform_user_id)
);

CREATE TABLE platform_link_event (
    id               TEXT COLLATE "C" PRIMARY KEY,
    user_id          TEXT COLLATE "C" NOT NULL REFERENCES "user"(user_id) ON DELETE CASCADE,
    platform         TEXT COLLATE "C" NOT NULL,
    platform_user_id TEXT COLLATE "C" NOT NULL,
    action           TEXT COLLATE "C" NOT NULL,
    at               TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_platform_link_event_user ON platform_link_event(user_id, at);
