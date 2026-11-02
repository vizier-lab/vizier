# Quickstart: Pluggable Storage Verification

Offline only: dummyplug agents, a local PostgreSQL with pgvector and MinIO in Docker. No model keys.

## 0. Services

```sh
docker run -d --name vz-pg -e POSTGRES_PASSWORD=vizier -p 55432:5432 pgvector/pgvector:pg17
docker exec vz-pg psql -U postgres -c 'CREATE DATABASE vizier' -c '\c vizier' -c 'CREATE EXTENSION vector'
docker run -d --name vz-minio -p 59000:9000 -e MINIO_ROOT_USER=vizier -e MINIO_ROOT_PASSWORD=vizier-secret minio/minio server /data
docker run --rm --network host --entrypoint sh minio/mc -c 'mc alias set l http://127.0.0.1:59000 vizier vizier-secret && mc mb l/vizier'
export VIZIER_DATABASE_URL=postgres://postgres:vizier@127.0.0.1:55432/vizier
export VIZIER_S3_ENDPOINT=http://127.0.0.1:59000 VIZIER_S3_BUCKET=vizier VIZIER_S3_REGION=us-east-1
export VIZIER_S3_ACCESS_KEY_ID=vizier VIZIER_S3_SECRET_ACCESS_KEY=vizier-secret
```

## 1. Automated gates

```sh
cargo test --workspace                       # SQLite + local only
VIZIER_TEST_DATABASE_URL=$VIZIER_DATABASE_URL VIZIER_TEST_S3_ENDPOINT=$VIZIER_S3_ENDPOINT \
  cargo test -p vizier-core storage::         # same suite on PostgreSQL + MinIO
cargo test -p vizier-core --no-default-features
cargo clippy --workspace --all-targets
```

Expected: identical pass counts for the backend-parametrised tests on each backend.

## 2. Default regression (US1, SC-001, SC-009)

```sh
cargo run -- run --workspace "$TMPDIR/vz-sqlite" --port 9990
```

With no PostgreSQL/S3 variables set, create a dummyplug agent via the API and in a WebUI chat send:

| Send | Expected |
|---|---|
| `tools` | dummyplug lists the tool catalogue |
| `memory_write` then `{"bundle":"notes","path":"hello","content":"pluggable storage works"}` | tool result `ok`; `$TMPDIR/vz-sqlite/agents/<id>/memory/notes/hello.md` exists |
| `memory_search` then `{"query":"pluggable"}` | the passage above is returned |

Upload a file in the chat, open its preview, restart, confirm history, memory and the file are still there.

## 3. PostgreSQL + S3 (US2, US3)

```sh
cargo run -- run --workspace "$TMPDIR/vz-pg" --port 9991 --storage postgres --objects s3
```

Repeat §2's table. Additionally:

- `psql "$VIZIER_DATABASE_URL" -c 'select version, name from vizier_schema_migrations'` lists the applied migrations.
- `mc ls -r l/vizier/agents/` shows `…/memory/notes/hello.md`; nothing under `$TMPDIR/vz-pg/agents/*/memory`.
- Upload a ≥ 100 MB file, `curl -r 1000-1999` its `/api/v1/files/<id>` URL: exactly 1000 bytes; RSS growth stays under 64 MB (SC-008).
- Promote it to an artifact, replace it, delete it: bucket keys follow (`content.v1` → `content.v2` → gone).
- Set an agent secret, then `psql … -c "select value from agent_secret"`: value starts with `enc:v1:` (FR-013).

## 4. Fail-closed startup (FR-004, SC-007)

| Change | Expected stderr (exit 1) |
|---|---|
| `unset VIZIER_DATABASE_URL` | `storage: VIZIER_DATABASE_URL is not set` |
| wrong password in URL | `storage: cannot connect to PostgreSQL (28P01)`, no password printed |
| DB without `vector` | `storage: PostgreSQL is missing the "vector" extension …` |
| wrong S3 secret | `objects: S3 bucket "vizier" is not reachable (403)` |

Grep all output for `vizier-secret` and the database password: zero hits.

## 5. Migration (US4, SC-006)

```sh
cargo run -- run --workspace "$TMPDIR/vz-sqlite" & sleep 5; kill %1   # §2 workspace, stopped
cargo run -- storage migrate --workspace "$TMPDIR/vz-sqlite" --storage postgres --objects s3 > report.json
```

Interrupt a second, fresh run (`--reset-destination`) with Ctrl-C during blob upload and rerun: it resumes and the report matches. A third run without `--reset-destination` exits 2. Start §3 on the migrated destination: the §2 conversation, memory passage and uploaded file are present and a new dummyplug turn works. The source workspace is byte-identical to before (`find … -type f | xargs sha256sum` compared).

## 6. Concurrency (SC-004)

`cargo test -p vizier-core storage::sql::tests::concurrent_history_seq -- --ignored` with `VIZIER_TEST_DATABASE_URL` set: 50 writers × 200 inserts, zero duplicate or missing `seq`; 10,000 racing `running → reporting` transitions, exactly one winner each.

## Cleanup

```sh
docker rm -f vz-pg vz-minio
```
