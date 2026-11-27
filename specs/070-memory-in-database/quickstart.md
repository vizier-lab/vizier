# Quickstart: Agent memory in the database

Every check below is runnable. Two constitution gates apply:
- **Manual verification**: this adds a startup storage migration.
- **E2E through dummyplug**: memory is agent-observable.

Agent-facing steps use the offline **dummyplug** provider (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`): send `tools`, then a tool name for a sample request, then the JSON request. In code mode the memory tools are called from an `execute_python` script.

## 0. Prepare

```powershell
just checkout 070-memory-in-database
cargo test -p vizier-core --lib
cargo test -p vizier-core --no-default-features
cargo clippy --workspace
```

## 1. Baseline, before the change (research Decision 9)

Run this on `master` and keep the numbers.

1. Start the binary with a dummyplug agent `bench`. Write 1,000 generated documents across 5 bundles (a script calling `POST /api/v1/agents/bench/memory`, bodies 1–6 KiB, with some `[[bundle/slug]]` links).
2. Time 20 runs each of:
   - `GET /api/v1/agents/bench/memory?limit=50`
   - `GET /api/v1/agents/bench/memory/bundles`
   - `GET /api/v1/agents/bench/memory/doc/default/<path>`
   - a dummyplug `memory_search` turn
3. Record the median of each.
4. *Optional, S3*: repeat on MinIO, with `docker run -p 9000:9000 minio/minio server /data` and the server's S3 env vars pointing at it.

## 2. Upgrade migration on a real workspace (manual, real binary)

1. Copy a pre-upgrade workspace with ≥ 3 agents, several bundles, at least one document over 8 KiB, one with cross-bundle links, and one with revision history. Snapshot for every agent:
   - `GET …/memory/bundles`
   - for each bundle, `GET …/memory?bundle=<b>&limit=1000`
   - for each document, `GET …/memory/doc/<b>/<p>` and `GET …/memory/history/<b>/<p>`
2. Plant two bad files in one agent's memory folder:
   - one with broken YAML frontmatter
   - one orphan folder `agents/no-such-agent/memory/default/x.md`
3. Start the new binary.
   - **Expect** one log line per agent: `memory migrated into the database: … migrated=<n> already=0 failed=1` (failed=1 only for the agent with the bad file).
   - **Expect** a completion line naming the folders that can be removed.
   - **Expect** no log line about `no-such-agent`.
4. Repeat every snapshot call from step 1.
   - **Expect** every response byte-identical to the snapshot (SC-001), except the broken file, which is absent.
   - **Expect** no new revision on any document (FR-013).
5. Confirm the source files are untouched: same file list and same hashes as before start (FR-017).
6. Restart.
   - **Expect** no migration log line (marker set).
   - **Expect** responses unchanged.
7. **Interrupted run** (SC-004): restore the copy and start the binary. Kill it mid-migration, using a large corpus or a debugger breakpoint after N documents. Start again.
   - **Expect** the same end state as step 4, with `already=<N>` on the agent that was cut.
8. *S3, optional*: repeat steps 1–4 with the workspace's `agents/` uploaded to MinIO.
   - Then stop MinIO and restart a fresh copy. **Expect** an `error` line saying the migration will retry, no marker, and the server still serving.
   - Start MinIO and restart. **Expect** completion.

**Fail conditions**:
- Any difference in step 4 other than the broken file.
- A new revision.
- A changed source file.
- A second migration run after the marker is set.

## 3. Agent memory works end to end (dummyplug)

Use an agent on dummyplug and run each step through `execute_python`.

| step | script | expect |
|---|---|---|
| write | `memory_write(bundle="notes", path="cats", title="Cats", content="Cats like [[notes/dogs]].")` | ok, no warning |
| read | `memory_read(bundle="notes", path="cats")` | title and content as written; `read_count` 1 more than before |
| link | `memory_write(bundle="notes", path="dogs", title="Dogs", content="Dogs.")`, then the related call on `notes/cats` | `dogs` listed; the edge is no longer broken |
| search | `memory_search(query="cats like")` | a passage from `notes/cats` with the text sliced correctly |
| identity | send any chat message | the `<memory bundle="core" path="soul">` block appears, as before (spec 043 quickstart §2) |
| delete | `memory_delete(bundle="notes", path="cats")`, then `memory_read` | not found; history keeps a deletion entry |
| rollback | `POST …/memory/history/notes/cats/rollback` to the content seq | document back; read works |

Then confirm that reads never touch the file store. Delete the agent's `agents/<id>/memory/` folder while the server runs, and repeat `read` and `search`.
- **Expect** identical results.
- **Expect** that no file reappears in the folder after any read or write.

## 4. Export and import (HTTP)

1. `GET …/memory/bundles/notes/export` downloads a zip.
   - **Expect** `index.md`, `log.md` (one line per revision, oldest first) and one `.md` per document with YAML frontmatter.
2. Import it into a second agent: `POST …/memory/bundles/import`.
   - **Expect** every document imported and searchable immediately.
3. Import a zip exported by the **previous** release (the checked-in fixture).
   - **Expect** the same document set.

## 5. SQLite → PostgreSQL (`vizier storage migrate`)

1. On the migrated SQLite workspace from §2, run `vizier storage migrate` to a PostgreSQL database.
2. Start on PostgreSQL. **Expect**:
   - no memory migration run (the marker was copied)
   - every §2 snapshot call to match

## 6. After-measurement

Repeat §1 on this branch.
- **Expect** at least a 5× lower median `memory_search` on MinIO (SC-003).
- **Expect** local timings no worse than the baseline.
- **Expect** the §2 migration of 1,000 local documents to finish in < 30 s (SC-005).

Record the numbers in the PR description.
