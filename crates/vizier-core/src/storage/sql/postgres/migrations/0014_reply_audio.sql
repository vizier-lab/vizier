-- Spec 055: a live voice reply's recording, kept beside the reply rather than in its history row
-- (the row is never rewritten). Filled on read: the reply reads as an audio reply.
CREATE TABLE IF NOT EXISTS reply_audio (
    history_uid TEXT PRIMARY KEY REFERENCES session_history(uid) ON DELETE CASCADE,
    attachment  TEXT NOT NULL,
    added_at    BIGINT NOT NULL
);
