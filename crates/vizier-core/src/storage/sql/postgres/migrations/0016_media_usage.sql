-- Spec 065: media calls (images, speech, transcription, embeddings) are usage events too.
-- Rows written before read as completions (`kind = 'text'`) with no media amount.
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS kind       TEXT COLLATE "C" NOT NULL DEFAULT 'text';
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS variant    TEXT COLLATE "C";
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS images     BIGINT NOT NULL DEFAULT 0;
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS characters BIGINT NOT NULL DEFAULT 0;
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS audio_ms   BIGINT NOT NULL DEFAULT 0;
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS measured   BIGINT NOT NULL DEFAULT 0;
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS local      BIGINT NOT NULL DEFAULT 0;
ALTER TABLE usage_event ADD COLUMN IF NOT EXISTS call_id    TEXT COLLATE "C";
CREATE INDEX IF NOT EXISTS usage_event_call ON usage_event (call_id) WHERE call_id IS NOT NULL;
