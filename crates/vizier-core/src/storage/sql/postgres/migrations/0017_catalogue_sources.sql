-- Spec 068: refreshed snapshots of the catalogue's sources other than models.dev (which keeps
-- `catalogue_snapshot`): one row per source, the trimmed upstream JSON gzipped.
CREATE TABLE IF NOT EXISTS catalogue_source_snapshot (
    source      TEXT COLLATE "C" PRIMARY KEY,
    schema      BIGINT NOT NULL,
    fetched_at  BIGINT NOT NULL,
    checked_at  BIGINT NOT NULL,
    last_error  TEXT COLLATE "C",
    failed_at   BIGINT,
    data        BYTEA NOT NULL
);
