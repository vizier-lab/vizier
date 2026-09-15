# Quickstart: Version History for CORE.md and Memories

**Feature**: `006-memory-version-history`

## Build & run

```sh
just install            # once: cargo fetch + npm i in webui/
cargo clippy            # must be clean
cargo test              # runs the new revision/diff unit tests
cd webui && npm run typecheck && cd ..
just run                # starts on dev.vizier.yaml (existing sqlite DB is upgraded in place:
                        # CREATE TABLE IF NOT EXISTS core_revision / memory_revision — no data migration)
```

Log in to the WebUI, pick (or create) an agent.

## Manual verification walkthrough (maps to spec user stories)

### US1 — every save is captured

1. **Agent save via conversation**: in chat, ask the agent to update its CORE ("add a line to your CORE saying you like tea"). It calls `WRITE_CORE`.
2. **User save via WebUI**: open *Core*, edit a line, Save.
3. **Memory**: ask the agent to remember something (`memory_write`), then edit that memory in *Memory* → view → Edit → Save; then delete it from the WebUI.
4. **Dream**: temporarily set the agent's `dream_interval` low, wait for a cycle that writes CORE/memory (or trigger via the dream API if available).
5. **No-op**: save CORE again without changing anything.

Check via API (replace `$TOKEN`, `$AGENT`):
```sh
curl -s -H "Authorization: Bearer $TOKEN" "localhost:8080/api/v1/agents/$AGENT/core/history" | jq
curl -s -H "Authorization: Bearer $TOKEN" "localhost:8080/api/v1/agents/$AGENT/memory/history/default/tea-preference" | jq
```
Expect: one row per save; `actor.type` = `agent` for steps 1/3a/4, `user` for 2/3b; `trigger.type` = `conversation`, `webui`, `dream` accordingly; the WebUI delete shows `deleted: true`; step 5 adds **no** row. A pre-existing CORE shows `seq: 1` as `trigger: baseline`, `actor: system`.

### US2 — browse & diff

1. *Core* page → **History** button → list opens newest-first, `Current` badge on top row.
2. Click a row → full content shown read-only.
3. Click **Changes** on the row from step US1.2 → diff vs previous; your edited line shows as delete + insert.
4. **Compare** → select the oldest and newest rows → diff spans all intermediate versions.
5. *Memory* → open a memory → **History** → same panel; a version's diff shows `title:`/`tags:` frontmatter lines when those changed.
6. API: `…/core/history?to=3` and `…/core/history?from=1&to=3` return `RevisionDiff` with hunks.
7. Log in as a user who can't view the agent → history endpoints return 403/404 exactly like `GET /core`.

### US3 — rollback

1. In CORE history, pick an older version → **Restore this version** → confirm dialog → confirm.
2. Expect: toast `Restored vK as vN`; list now has a new top row with `trigger: rollback (restored from vK)`, `actor: user`; older rows untouched; editor shows the restored content.
3. Ask the agent to `READ_CORE` → it returns the restored content.
4. Restore the version that is already current → toast "nothing changed", no new row.
5. Memory: delete a memory from the WebUI, open its history from the API (`…/memory/history/{bundle}/{path}`), `POST … {"seq": <last content version>}` → memory reappears in the list and graph; history shows the deletion row followed by the rollback row.
6. Delete the agent → both `SELECT COUNT(*) FROM core_revision WHERE agent_id = '$AGENT'` and `… FROM memory_revision …` in the workspace sqlite file return 0.

### Import / export unchanged

Export a bundle → the zip contains only concept/`index.md`/`log.md` files (no history). Import it into another agent → each imported concept's history starts with a `trigger: import` row.

## Unit tests to expect (`cargo test`)

- `storage::sqlite::core_revision`: seq numbering; no-op skip; baseline insertion on first record and on first list; agent cascade delete.
- `storage::sqlite::memory_revision`: same as above plus deletion entry then restore; `memory_canonical` ↔ `parse_memory_canonical` round-trip.
- `storage::diff`: `diff_lines` insert/delete/equal + `\r\n` normalization + identical input ⇒ zero hunks.
- `schema::revision`: `RevisionOrigin::from_session` maps `Dream` ⇒ `dream`, others ⇒ `conversation`.
