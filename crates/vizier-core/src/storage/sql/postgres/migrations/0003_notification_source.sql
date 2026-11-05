-- Where a notification came from, and a short excerpt of what it says
-- (specs/051-realtime-notifications data-model.md).
--
-- `source_session` is the originating session as JSON, used to fill the row's title on read and
-- to delete the row with its conversation. `source_title` is a stored title for a source with no
-- session (a task). `excerpt` is plain text, at most 140 characters plus an ellipsis. Existing
-- rows keep NULL and read as their summary.

ALTER TABLE notification ADD COLUMN IF NOT EXISTS source_session TEXT COLLATE "C";
ALTER TABLE notification ADD COLUMN IF NOT EXISTS source_title TEXT;
ALTER TABLE notification ADD COLUMN IF NOT EXISTS excerpt TEXT;

-- Discord and Telegram no longer notify: drop the hourly aggregate rows an older build wrote.
-- notification_read rows cascade.
DELETE FROM notification WHERE category = 'channel_replies';
