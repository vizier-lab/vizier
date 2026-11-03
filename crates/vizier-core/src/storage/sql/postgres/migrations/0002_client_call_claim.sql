-- The deferred client-call claim is a lease on the stored row, not a status
-- (specs/163-agent-service-seam research R7): `claimed_by` names the gateway delivering the
-- call and `claimed_at` says when it took the claim, so a claim older than the claim
-- timeout is free again. The status vocabulary the WebUI reads does not change.
--
-- `0001_initial.sql` is never edited - an applied migration is skipped by the runner - so
-- the columns an existing deployment misses arrive here.

ALTER TABLE client_tool_call ADD COLUMN IF NOT EXISTS claimed_by TEXT COLLATE "C";
ALTER TABLE client_tool_call ADD COLUMN IF NOT EXISTS claimed_at TEXT COLLATE "C";
