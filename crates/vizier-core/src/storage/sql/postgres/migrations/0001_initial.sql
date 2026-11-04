-- The whole SQLite schema (`storage/sqlite/mod.rs` `init_schema` and every `init_*_schema`,
-- `storage/sqlite/project.rs` `init_project_schema`), translated for PostgreSQL
-- (specs/045-pluggable-storage/data-model.md). Table and column names are identical on both
-- backends: encryption binds them into the AAD. Integers (ids, ms timestamps, counts, 0/1
-- booleans) are BIGINT, reals DOUBLE PRECISION, bytes BYTEA. Every text column is
-- `COLLATE "C"`, so ordering, comparison and uniqueness follow bytes exactly like SQLite's
-- BINARY collation, whatever the database's default locale.
--
-- `rowid BIGINT GENERATED ALWAYS AS IDENTITY` stands in for SQLite's implicit rowid on the
-- tables whose reads break ties in insertion order (`ORDER BY …, rowid`).

-- ---- memory (legacy flat memories) -------------------------------------------------------

CREATE TABLE memory (
    id TEXT COLLATE "C" PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    slug TEXT COLLATE "C" NOT NULL,
    visibility TEXT COLLATE "C" NOT NULL,
    data TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_memory_agent ON memory(agent_id);
CREATE INDEX idx_memory_visibility ON memory(visibility);

CREATE TABLE task (
    id TEXT COLLATE "C" PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    slug TEXT COLLATE "C" NOT NULL,
    is_active BIGINT NOT NULL DEFAULT 1,
    data TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_task_agent ON task(agent_id);

CREATE TABLE session_detail (
    id TEXT COLLATE "C" PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    channel TEXT COLLATE "C" NOT NULL,
    topic TEXT COLLATE "C",
    data TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_sd_agent ON session_detail(agent_id);

CREATE TABLE "user" (
    user_id TEXT COLLATE "C" PRIMARY KEY,
    username TEXT COLLATE "C" NOT NULL UNIQUE,
    data TEXT COLLATE "C" NOT NULL,
    last_seen_at TEXT COLLATE "C",
    previous_visit_at TEXT COLLATE "C"
);

CREATE TABLE user_profile (
    user_id TEXT COLLATE "C" PRIMARY KEY,
    data TEXT COLLATE "C" NOT NULL
);

CREATE TABLE role (
    role_id TEXT COLLATE "C" PRIMARY KEY,
    is_system BIGINT NOT NULL DEFAULT 0,
    data TEXT COLLATE "C" NOT NULL
);

CREATE TABLE api_key (
    id TEXT COLLATE "C" PRIMARY KEY,
    user_id TEXT COLLATE "C" NOT NULL,
    key_hash TEXT COLLATE "C" NOT NULL UNIQUE,
    data TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_apikey_user ON api_key(user_id);
CREATE INDEX idx_apikey_hash ON api_key(key_hash);

CREATE TABLE agent_config (
    agent_id TEXT COLLATE "C" PRIMARY KEY,
    data TEXT COLLATE "C" NOT NULL
);

CREATE TABLE agent_core (
    agent_id TEXT COLLATE "C" PRIMARY KEY,
    content TEXT COLLATE "C" NOT NULL
);

CREATE TABLE provider_config (
    variant TEXT COLLATE "C" PRIMARY KEY,
    data TEXT COLLATE "C" NOT NULL
);

CREATE TABLE custom_provider (
    id TEXT COLLATE "C" PRIMARY KEY,
    data TEXT COLLATE "C" NOT NULL
);

CREATE TABLE catalogue_snapshot (
    id          BIGINT PRIMARY KEY CHECK (id = 1),
    schema      BIGINT NOT NULL,
    source_url  TEXT COLLATE "C" NOT NULL,
    etag        TEXT COLLATE "C",
    fetched_at  BIGINT NOT NULL,
    checked_at  BIGINT NOT NULL,
    last_error  TEXT COLLATE "C",
    failed_at   BIGINT,
    data        BYTEA NOT NULL
);

CREATE TABLE catalogue_logo (
    id          TEXT COLLATE "C" PRIMARY KEY,
    svg         BYTEA,
    fetched_at  BIGINT NOT NULL
);

CREATE TABLE global_config (
    key TEXT COLLATE "C" PRIMARY KEY,
    data TEXT COLLATE "C" NOT NULL
);

CREATE TABLE dream_journal (
    id TEXT COLLATE "C" PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    dream_cycle_id TEXT COLLATE "C",
    stage TEXT COLLATE "C" NOT NULL,
    timestamp BIGINT NOT NULL,
    data TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_dj_agent ON dream_journal(agent_id);
CREATE INDEX idx_dj_cycle ON dream_journal(dream_cycle_id);

CREATE TABLE state (
    key TEXT COLLATE "C" PRIMARY KEY,
    value TEXT COLLATE "C" NOT NULL
);

CREATE TABLE session_file (
    id TEXT COLLATE "C" PRIMARY KEY,
    session_slug TEXT COLLATE "C" NOT NULL,
    agent_id TEXT COLLATE "C" NOT NULL,
    data TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_sf_session ON session_file(session_slug, agent_id);

-- ---- artifacts (spec 035) ----------------------------------------------------------------

CREATE TABLE artifact (
    id            TEXT COLLATE "C" PRIMARY KEY,
    agent_id      TEXT COLLATE "C" NOT NULL,
    name          TEXT COLLATE "C" NOT NULL,
    kind          TEXT COLLATE "C" NOT NULL,
    mime_type     TEXT COLLATE "C" NOT NULL,
    size          BIGINT NOT NULL,
    version       BIGINT NOT NULL,
    content_file  TEXT COLLATE "C" NOT NULL,
    origin_channel TEXT COLLATE "C",
    origin_topic  TEXT COLLATE "C",
    source_filename TEXT COLLATE "C",
    created_by    TEXT COLLATE "C" NOT NULL,
    created_at    BIGINT NOT NULL,
    updated_at    BIGINT NOT NULL,
    UNIQUE (agent_id, name)
);
CREATE INDEX idx_artifact_agent_updated ON artifact(agent_id, updated_at DESC, id DESC);
CREATE INDEX idx_artifact_agent_kind ON artifact(agent_id, kind, updated_at DESC, id DESC);

CREATE TABLE artifact_event (
    id            TEXT COLLATE "C" PRIMARY KEY,
    agent_id      TEXT COLLATE "C" NOT NULL,
    artifact_id   TEXT COLLATE "C" NOT NULL,
    artifact_name TEXT COLLATE "C" NOT NULL,
    action        TEXT COLLATE "C" NOT NULL,
    actor         TEXT COLLATE "C" NOT NULL,
    source_channel TEXT COLLATE "C",
    source_topic  TEXT COLLATE "C",
    source_filename TEXT COLLATE "C",
    created_at    BIGINT NOT NULL
);
CREATE INDEX idx_artifact_event_agent_time ON artifact_event(agent_id, created_at DESC, id DESC);
CREATE INDEX idx_artifact_event_artifact ON artifact_event(artifact_id, created_at);

CREATE TABLE artifact_reference (
    artifact_id  TEXT COLLATE "C" NOT NULL,
    agent_id     TEXT COLLATE "C" NOT NULL,
    kind         TEXT COLLATE "C" NOT NULL,
    target       TEXT COLLATE "C" NOT NULL,
    session_slug TEXT COLLATE "C",
    history_uid  TEXT COLLATE "C",
    channel      TEXT COLLATE "C",
    topic        TEXT COLLATE "C",
    bundle       TEXT COLLATE "C",
    path         TEXT COLLATE "C",
    label        TEXT COLLATE "C" NOT NULL,
    created_at   BIGINT NOT NULL,
    PRIMARY KEY (artifact_id, kind, target)
);
CREATE INDEX idx_artifact_ref_artifact ON artifact_reference(artifact_id, kind, created_at);
CREATE INDEX idx_artifact_ref_session ON artifact_reference(kind, agent_id, session_slug);
CREATE INDEX idx_artifact_ref_memory ON artifact_reference(kind, agent_id, bundle, path);

-- ---- session history and reactions -------------------------------------------------------

CREATE TABLE session_history (
    uid TEXT COLLATE "C" PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    channel TEXT COLLATE "C" NOT NULL,
    topic TEXT COLLATE "C",
    timestamp BIGINT NOT NULL,
    content_type TEXT COLLATE "C" NOT NULL,
    data TEXT COLLATE "C" NOT NULL,
    -- SQLite assigns MAX(seq) + 1 inside the write; here the identity does (data-model.md).
    seq BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE
);
CREATE INDEX idx_sh_session ON session_history(agent_id, channel, topic);
CREATE INDEX idx_sh_time ON session_history(timestamp);
CREATE INDEX idx_sh_agent_time ON session_history(agent_id, timestamp);

CREATE TABLE message_reaction (
    history_uid  TEXT COLLATE "C" NOT NULL REFERENCES session_history(uid) ON DELETE CASCADE,
    reactor_id   TEXT COLLATE "C" NOT NULL,
    reactor_name TEXT COLLATE "C",
    emoji        TEXT COLLATE "C" NOT NULL,
    added_at     BIGINT NOT NULL,
    rowid        BIGINT GENERATED ALWAYS AS IDENTITY,
    PRIMARY KEY (history_uid, reactor_id, emoji)
);
CREATE INDEX idx_reaction_uid ON message_reaction(history_uid);

CREATE TABLE platform_message_link (
    agent_id    TEXT COLLATE "C" NOT NULL,
    platform    TEXT COLLATE "C" NOT NULL,
    chat_id     TEXT COLLATE "C" NOT NULL,
    message_id  TEXT COLLATE "C" NOT NULL,
    history_uid TEXT COLLATE "C" NOT NULL REFERENCES session_history(uid) ON DELETE CASCADE,
    PRIMARY KEY (agent_id, platform, chat_id, message_id)
);

-- ---- memory graph (derived) --------------------------------------------------------------

CREATE TABLE memory_node (
    agent_id TEXT COLLATE "C" NOT NULL,
    bundle TEXT COLLATE "C" NOT NULL,
    path TEXT COLLATE "C" NOT NULL,
    slug TEXT COLLATE "C" NOT NULL,
    title TEXT COLLATE "C" NOT NULL,
    tags_json TEXT COLLATE "C" NOT NULL,
    attachment_count BIGINT NOT NULL DEFAULT 0,
    created_at TEXT COLLATE "C" NOT NULL,
    updated_at TEXT COLLATE "C" NOT NULL,
    read_count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (agent_id, bundle, path)
);
CREATE INDEX idx_memory_node_agent_bundle ON memory_node(agent_id, bundle);

CREATE TABLE memory_edge (
    agent_id TEXT COLLATE "C" NOT NULL,
    source_bundle TEXT COLLATE "C" NOT NULL,
    source_path TEXT COLLATE "C" NOT NULL,
    target_bundle TEXT COLLATE "C" NOT NULL,
    target_path TEXT COLLATE "C",
    target_kind TEXT COLLATE "C" NOT NULL,
    broken BIGINT NOT NULL DEFAULT 0
);
CREATE INDEX idx_memory_edge_source ON memory_edge(agent_id, source_bundle, source_path);
CREATE INDEX idx_memory_edge_target ON memory_edge(agent_id, target_bundle, target_path);

CREATE TABLE memory_passage (
    agent_id TEXT COLLATE "C" NOT NULL,
    bundle TEXT COLLATE "C" NOT NULL,
    path TEXT COLLATE "C" NOT NULL,
    ordinal BIGINT NOT NULL,
    line_start BIGINT NOT NULL,
    line_end BIGINT NOT NULL,
    char_start BIGINT NOT NULL,
    char_end BIGINT NOT NULL,
    continues BIGINT NOT NULL DEFAULT 0,
    content_hash TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (agent_id, bundle, path, ordinal)
);
CREATE INDEX idx_memory_passage_doc ON memory_passage(agent_id, bundle, path);

-- ---- revision history (spec 006) ---------------------------------------------------------

CREATE TABLE core_revision (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    seq BIGINT NOT NULL,
    content TEXT COLLATE "C" NOT NULL,
    actor_kind TEXT COLLATE "C" NOT NULL,
    actor_id TEXT COLLATE "C",
    actor_name TEXT COLLATE "C",
    trigger TEXT COLLATE "C" NOT NULL,
    restored_from BIGINT,
    created_at BIGINT NOT NULL,
    dream_cycle_id TEXT COLLATE "C",
    UNIQUE (agent_id, seq)
);
CREATE INDEX idx_core_rev_doc ON core_revision(agent_id, seq DESC);
CREATE INDEX idx_core_rev_cycle ON core_revision(agent_id, dream_cycle_id);

CREATE TABLE memory_revision (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    bundle TEXT COLLATE "C" NOT NULL,
    path TEXT COLLATE "C" NOT NULL,
    seq BIGINT NOT NULL,
    content TEXT COLLATE "C",
    deleted BIGINT NOT NULL DEFAULT 0,
    actor_kind TEXT COLLATE "C" NOT NULL,
    actor_id TEXT COLLATE "C",
    actor_name TEXT COLLATE "C",
    trigger TEXT COLLATE "C" NOT NULL,
    restored_from BIGINT,
    created_at BIGINT NOT NULL,
    dream_cycle_id TEXT COLLATE "C",
    UNIQUE (agent_id, bundle, path, seq)
);
CREATE INDEX idx_mem_rev_doc ON memory_revision(agent_id, bundle, path, seq DESC);
CREATE INDEX idx_mem_rev_agent ON memory_revision(agent_id);
CREATE INDEX idx_mem_rev_cycle ON memory_revision(agent_id, dream_cycle_id);

-- ---- task runs (spec 011) ----------------------------------------------------------------

CREATE TABLE task_run (
    id           BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    agent_id     TEXT COLLATE "C" NOT NULL,
    task_slug    TEXT COLLATE "C" NOT NULL,
    ran_at       BIGINT NOT NULL,
    finished_at  BIGINT,
    session_key  TEXT COLLATE "C" NOT NULL,
    state        TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_task_run_task ON task_run(agent_id, task_slug, ran_at DESC, id DESC);
CREATE INDEX idx_task_run_state ON task_run(state);

-- ---- background jobs (spec 012) ----------------------------------------------------------

CREATE TABLE background_job (
    id             TEXT COLLATE "C" PRIMARY KEY,
    kind           TEXT COLLATE "C" NOT NULL,
    origin_agent   TEXT COLLATE "C" NOT NULL,
    origin_channel TEXT COLLATE "C" NOT NULL,
    origin_topic   TEXT COLLATE "C",
    depth          BIGINT NOT NULL,
    timeout_secs   BIGINT NOT NULL,
    created_at     BIGINT NOT NULL,
    finished_at    BIGINT,
    state          TEXT COLLATE "C" NOT NULL,
    cancelled_by   TEXT COLLATE "C",
    reason         TEXT COLLATE "C"
);
CREATE INDEX idx_bg_job_origin ON background_job(origin_agent, origin_channel, origin_topic, state);
CREATE INDEX idx_bg_job_agent ON background_job(origin_agent, state);
CREATE INDEX idx_bg_job_state ON background_job(state);

CREATE TABLE background_piece (
    job_id          TEXT COLLATE "C" NOT NULL REFERENCES background_job(id) ON DELETE CASCADE,
    ordinal         BIGINT NOT NULL,
    prompt          TEXT COLLATE "C" NOT NULL,
    executor_agent  TEXT COLLATE "C" NOT NULL,
    session_channel TEXT COLLATE "C" NOT NULL,
    session_topic   TEXT COLLATE "C" NOT NULL,
    started_at      BIGINT NOT NULL,
    finished_at     BIGINT,
    state           TEXT COLLATE "C" NOT NULL,
    reason          TEXT COLLATE "C",
    PRIMARY KEY (job_id, ordinal)
);
CREATE INDEX idx_bg_piece_state ON background_piece(state);

-- ---- integrations (specs 016, 017) -------------------------------------------------------

CREATE TABLE integration (
    id          TEXT COLLATE "C" PRIMARY KEY,
    kind        TEXT COLLATE "C" NOT NULL,
    family      TEXT COLLATE "C" NOT NULL,
    name        TEXT COLLATE "C" NOT NULL,
    settings    TEXT COLLATE "C" NOT NULL,
    enabled     BIGINT NOT NULL,
    scope       TEXT COLLATE "C" NOT NULL,
    created_by  TEXT COLLATE "C",
    created_at  TEXT COLLATE "C" NOT NULL,
    updated_at  TEXT COLLATE "C" NOT NULL,
    creator     TEXT COLLATE "C"
);

CREATE TABLE integration_attachment (
    integration_id TEXT COLLATE "C" NOT NULL REFERENCES integration(id) ON DELETE CASCADE,
    agent_id       TEXT COLLATE "C" NOT NULL,
    created_at     TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (integration_id, agent_id)
);
CREATE INDEX idx_integration_attachment_agent ON integration_attachment(agent_id);

CREATE TABLE integration_grant (
    agent_id    TEXT COLLATE "C" NOT NULL,
    family      TEXT COLLATE "C" NOT NULL,
    granted_by  TEXT COLLATE "C" NOT NULL,
    granted_at  TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (agent_id, family)
);

CREATE TABLE integration_event (
    id               TEXT COLLATE "C" PRIMARY KEY,
    agent_id         TEXT COLLATE "C" NOT NULL,
    integration_id   TEXT COLLATE "C",
    integration_name TEXT COLLATE "C" NOT NULL,
    family           TEXT COLLATE "C" NOT NULL,
    actor            TEXT COLLATE "C" NOT NULL,
    action           TEXT COLLATE "C" NOT NULL,
    fields           TEXT COLLATE "C" NOT NULL DEFAULT '[]',
    session          TEXT COLLATE "C",
    at               TEXT COLLATE "C" NOT NULL
);
CREATE INDEX idx_integration_event_agent ON integration_event(agent_id, at DESC);
CREATE INDEX idx_integration_event_integration ON integration_event(integration_id, at DESC);

-- ---- usage accounting (spec 018) ---------------------------------------------------------

CREATE TABLE usage_event (
    id                          BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    agent_id                    TEXT COLLATE "C"    NOT NULL,
    at                          BIGINT  NOT NULL,
    activity                    TEXT COLLATE "C"    NOT NULL,
    channel                     TEXT COLLATE "C",
    topic                       TEXT COLLATE "C",
    place                       TEXT COLLATE "C"    NOT NULL,
    turn_id                     TEXT COLLATE "C",
    round                       BIGINT,
    provider                    TEXT COLLATE "C"    NOT NULL DEFAULT '',
    model                       TEXT COLLATE "C"    NOT NULL DEFAULT '',
    served_model                TEXT COLLATE "C",
    input_tokens                BIGINT  NOT NULL DEFAULT 0,
    cached_input_tokens         BIGINT  NOT NULL DEFAULT 0,
    cache_creation_input_tokens BIGINT  NOT NULL DEFAULT 0,
    output_tokens               BIGINT  NOT NULL DEFAULT 0,
    duration_ms                 BIGINT  NOT NULL DEFAULT 0,
    legacy                      BIGINT  NOT NULL DEFAULT 0
);
CREATE INDEX usage_event_agent_at ON usage_event (agent_id, at);
CREATE INDEX usage_event_agent_conv ON usage_event (agent_id, channel, topic);
CREATE INDEX usage_event_turn ON usage_event (turn_id) WHERE turn_id IS NOT NULL;

CREATE TABLE usage_failed_turn (
    turn_id  TEXT COLLATE "C"    PRIMARY KEY,
    agent_id TEXT COLLATE "C"    NOT NULL,
    at       BIGINT  NOT NULL
);
CREATE INDEX usage_failed_turn_agent ON usage_failed_turn (agent_id);

-- ---- client tools (spec 019) -------------------------------------------------------------

CREATE TABLE client_tool_call (
    history_uid      TEXT COLLATE "C" PRIMARY KEY,
    agent_id         TEXT COLLATE "C" NOT NULL,
    channel          TEXT COLLATE "C" NOT NULL,
    topic            TEXT COLLATE "C",
    status           TEXT COLLATE "C" NOT NULL,
    detail           TEXT COLLATE "C",
    state            TEXT COLLATE "C",
    state_updated_at TEXT COLLATE "C",
    created_at       TEXT COLLATE "C" NOT NULL,
    finished_at      TEXT COLLATE "C",
    rowid            BIGINT GENERATED ALWAYS AS IDENTITY
);
CREATE INDEX idx_client_call_session ON client_tool_call(agent_id, channel, topic);
CREATE INDEX idx_client_call_pending ON client_tool_call(status) WHERE status = 'pending';

-- ---- questions (spec 042) ----------------------------------------------------------------

CREATE TABLE question_set (
    set_id     TEXT COLLATE "C" PRIMARY KEY,
    short_id   TEXT COLLATE "C" NOT NULL UNIQUE,
    agent_id   TEXT COLLATE "C" NOT NULL,
    channel    TEXT COLLATE "C" NOT NULL,
    channel_json TEXT COLLATE "C" NOT NULL,
    topic      TEXT COLLATE "C",
    state      TEXT COLLATE "C" NOT NULL,
    questions  TEXT COLLATE "C" NOT NULL,
    answers    TEXT COLLATE "C",
    answerer   TEXT COLLATE "C",
    controls   TEXT COLLATE "C",
    created_at BIGINT NOT NULL,
    closed_at  BIGINT,
    rowid      BIGINT GENERATED ALWAYS AS IDENTITY
);
CREATE INDEX idx_qs_session ON question_set(agent_id, channel, topic);
CREATE UNIQUE INDEX idx_qs_short ON question_set(short_id);

CREATE TABLE sensitive_ask (
    ask_id       TEXT COLLATE "C" PRIMARY KEY,
    agent_id     TEXT COLLATE "C" NOT NULL,
    channel      TEXT COLLATE "C" NOT NULL,
    channel_json TEXT COLLATE "C" NOT NULL,
    topic        TEXT COLLATE "C",
    purpose      TEXT COLLATE "C" NOT NULL,
    label        TEXT COLLATE "C",
    state        TEXT COLLATE "C" NOT NULL,
    requested_at BIGINT NOT NULL,
    closed_at    BIGINT,
    rowid        BIGINT GENERATED ALWAYS AS IDENTITY
);
CREATE INDEX idx_sa_session ON sensitive_ask(agent_id, channel, topic, state);
CREATE INDEX idx_sa_open ON sensitive_ask(state) WHERE state = 'open';

-- ---- scratchpad (spec 020) ---------------------------------------------------------------

CREATE TABLE scratch_file (
    agent_id   TEXT COLLATE "C"    NOT NULL,
    channel    TEXT COLLATE "C"    NOT NULL,
    topic      TEXT COLLATE "C"    NOT NULL DEFAULT '',
    name       TEXT COLLATE "C"    NOT NULL,
    content    TEXT COLLATE "C"    NOT NULL,
    size       BIGINT  NOT NULL,
    updated_at TEXT COLLATE "C"    NOT NULL,
    PRIMARY KEY (agent_id, channel, topic, name)
);

-- ---- pending turns (spec 023) ------------------------------------------------------------

CREATE TABLE session_turn (
    id              BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    agent_id        TEXT COLLATE "C"    NOT NULL,
    channel         TEXT COLLATE "C"    NOT NULL,
    topic           TEXT COLLATE "C"    NOT NULL DEFAULT '',
    session         TEXT COLLATE "C"    NOT NULL,
    request         TEXT COLLATE "C"    NOT NULL,
    state           TEXT COLLATE "C"    NOT NULL,
    request_uid     TEXT COLLATE "C",
    accepted_at     TEXT COLLATE "C"    NOT NULL,
    started_at      TEXT COLLATE "C",
    resume_count    BIGINT  NOT NULL DEFAULT 0,
    steps_at_resume BIGINT  NOT NULL DEFAULT 0
);
CREATE INDEX idx_session_turn_agent ON session_turn (agent_id, id);

-- ---- agent secrets (spec 027) ------------------------------------------------------------

CREATE TABLE agent_secret (
    agent_id   TEXT COLLATE "C" NOT NULL,
    name       TEXT COLLATE "C" NOT NULL,
    value      TEXT COLLATE "C" NOT NULL,
    created_at TEXT COLLATE "C" NOT NULL,
    updated_at TEXT COLLATE "C" NOT NULL,
    updated_by TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (agent_id, name)
);

CREATE TABLE agent_secret_event (
    id       TEXT COLLATE "C" PRIMARY KEY,
    agent_id TEXT COLLATE "C" NOT NULL,
    name     TEXT COLLATE "C" NOT NULL,
    action   TEXT COLLATE "C" NOT NULL,
    actor    TEXT COLLATE "C" NOT NULL,
    at       TEXT COLLATE "C" NOT NULL,
    rowid    BIGINT GENERATED ALWAYS AS IDENTITY
);
CREATE INDEX idx_agent_secret_event_agent ON agent_secret_event(agent_id, at DESC);

-- ---- media catalogue (spec 028) ----------------------------------------------------------

CREATE TABLE media_catalogue_snapshot (
    capability  TEXT COLLATE "C" PRIMARY KEY CHECK (capability IN ('speech', 'transcription', 'image')),
    schema      BIGINT NOT NULL,
    fetched_at  BIGINT NOT NULL,
    checked_at  BIGINT NOT NULL,
    last_error  TEXT COLLATE "C",
    failed_at   BIGINT,
    data        BYTEA NOT NULL
);

-- ---- notifications (spec 029) ------------------------------------------------------------

CREATE TABLE notification (
    id               BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    event_key        TEXT COLLATE "C" NOT NULL UNIQUE,
    agent_id         TEXT COLLATE "C" NOT NULL,
    audience_user_id TEXT COLLATE "C",
    category         TEXT COLLATE "C" NOT NULL,
    tone             TEXT COLLATE "C" NOT NULL,
    summary          TEXT COLLATE "C" NOT NULL,
    href             TEXT COLLATE "C" NOT NULL,
    created_at       BIGINT NOT NULL
);
CREATE INDEX idx_notification_created ON notification(created_at DESC, id DESC);
CREATE INDEX idx_notification_agent_created ON notification(agent_id, created_at DESC, id DESC);

CREATE TABLE notification_read (
    user_id         TEXT COLLATE "C" NOT NULL,
    notification_id BIGINT NOT NULL REFERENCES notification(id) ON DELETE CASCADE,
    read_at         BIGINT NOT NULL,
    PRIMARY KEY (user_id, notification_id)
);

-- ---- projects (specs 040, 041) -----------------------------------------------------------

CREATE TABLE project (
    id            TEXT COLLATE "C" PRIMARY KEY,
    agent_id      TEXT COLLATE "C" NOT NULL,
    name          TEXT COLLATE "C" NOT NULL,
    folder        TEXT COLLATE "C" NOT NULL,
    owner_id      TEXT COLLATE "C" NOT NULL,
    shell_enabled BIGINT NOT NULL DEFAULT 1,
    created_at    BIGINT NOT NULL,
    removed_at    BIGINT,
    UNIQUE (agent_id, folder)
);
CREATE INDEX idx_project_agent ON project(agent_id, removed_at);

CREATE TABLE project_chat (
    agent_id   TEXT COLLATE "C"    NOT NULL,
    channel    TEXT COLLATE "C"    NOT NULL,
    topic      TEXT COLLATE "C"    NOT NULL,
    mode       TEXT COLLATE "C"    NOT NULL CHECK (mode IN ('plan', 'build')),
    updated_at BIGINT  NOT NULL,
    PRIMARY KEY (agent_id, channel, topic)
);

CREATE TABLE project_worktree (
    id          TEXT COLLATE "C"    PRIMARY KEY,
    project_id  TEXT COLLATE "C"    NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    agent_id    TEXT COLLATE "C"    NOT NULL,
    owner_id    TEXT COLLATE "C"    NOT NULL,
    owner_name  TEXT COLLATE "C"    NOT NULL,
    topic       TEXT COLLATE "C"    NOT NULL,
    name        TEXT COLLATE "C"    NOT NULL,
    branch      TEXT COLLATE "C"    NOT NULL,
    base_ref    TEXT COLLATE "C"    NOT NULL,
    path        TEXT COLLATE "C"    NOT NULL,
    created_at  BIGINT  NOT NULL,
    UNIQUE (project_id, name),
    UNIQUE (project_id, topic)
);
CREATE INDEX idx_worktree_project ON project_worktree(project_id);
