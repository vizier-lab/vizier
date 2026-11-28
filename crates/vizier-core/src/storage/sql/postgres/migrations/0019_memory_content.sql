-- Spec 070: a memory document's body lives beside its metadata. NULL means "not here": a row
-- the one-time file migration could not fill (migration.memory_in_database_v1).
ALTER TABLE memory_node ADD COLUMN IF NOT EXISTS content TEXT COLLATE "C";
