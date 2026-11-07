-- Language servers in project chats (specs/054-project-lsp data-model). The same shape as the
-- SQLite `init_project_schema`: the switch (existing projects read off) and per-language overrides.

ALTER TABLE project ADD COLUMN lsp_enabled BIGINT NOT NULL DEFAULT 0;

CREATE TABLE project_lsp_override (
    project_id TEXT COLLATE "C" NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    language   TEXT COLLATE "C" NOT NULL,
    command    TEXT NOT NULL,
    args       TEXT NOT NULL,
    env        TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (project_id, language)
);
