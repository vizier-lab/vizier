# 2.6 Storage & Shell

## Storage

Vizier stores two kinds of data, and each has its own backend:

- **Database**: agents, sessions, history, tasks, users/roles/API keys, providers, dream journals, file records, the memory graph index and the memory vectors.
- **Files**: uploads, artifacts, and memory documents (markdown, see [Memory](./memory.md)).

The defaults need no setup and no external service:

```yaml
vizier:
  storage:
    type: sqlite    # <workspace>/.runtime/vizier.db, vectors with sqlite-vec
  objects:
    type: local     # files under the workspace directory
```

A server deployment can move either one to a managed service. The two choices are independent.

| `storage.type` | What it is |
|---|---|
| `sqlite` | Default. Embedded, one file. |
| `postgres` | PostgreSQL 15 or later with the [`vector`](https://github.com/pgvector/pgvector) extension. |
| `filesystem` | Legacy. Still parses so old configs load; on startup Vizier logs a warning and **migrates** the old flat files into SQLite (one-time, idempotent). Rejected on the CLI. |

| `objects.type` | What it is |
|---|---|
| `local` | Default. Files under the workspace (`uploads/`, `artifacts/`, `agents/<id>/memory/`). |
| `s3` | Any S3-compatible bucket (AWS S3, MinIO, Cloudflare R2, …). |

The desktop app always uses `sqlite` and `local`.

Credentials and secret values in the database are encrypted at rest with the workspace key on every backend; back up the key separately from the database — see [Encryption at Rest](./encryption.md).

### PostgreSQL

```yaml
vizier:
  storage:
    type: postgres
    url_env: VIZIER_DATABASE_URL   # default: the variable that holds the URL
    pool_size: 8                   # default
    ca_file: /run/secrets/pg-ca.pem   # optional; the system's roots otherwise
```

The connection URL is read only from the environment, never from the config file or the command line:

```sh
export VIZIER_DATABASE_URL='postgres://vizier:…@db.internal:5432/vizier?sslmode=require'
vizier run --storage postgres
```

Before the first start, create the database and enable the extension as a superuser: `CREATE EXTENSION vector;`. Vizier creates and upgrades its own tables at startup (recorded in `vizier_schema_migrations`) and refuses to start, with one of these messages, when something is wrong:

| Message | Fix |
|---|---|
| `storage: VIZIER_DATABASE_URL is not set` | Export the URL |
| `storage: cannot connect to PostgreSQL (<code>)` | Check host, credentials (`28P01`), database name (`3D000`) |
| `storage: PostgreSQL is missing the "vector" extension` | `CREATE EXTENSION vector;` |
| `storage: database schema version N is newer than this binary (M)` | Upgrade Vizier |

### S3-compatible files

```yaml
vizier:
  objects:
    type: s3
    bucket: vizier
    region: us-east-1
    endpoint: https://minio.internal:9000   # optional; AWS when absent
    path_style: true                        # default: true when endpoint is set
    prefix: prod                            # optional key prefix inside the bucket
```

Credentials come from `VIZIER_S3_ACCESS_KEY_ID`, `VIZIER_S3_SECRET_ACCESS_KEY` and optionally `VIZIER_S3_SESSION_TOKEN` (or the standard `AWS_*` variables). `VIZIER_S3_BUCKET`, `VIZIER_S3_REGION`, `VIZIER_S3_ENDPOINT`, `VIZIER_S3_PATH_STYLE` and `VIZIER_S3_PREFIX` override the YAML values. At startup Vizier writes, reads and deletes a probe object and refuses to start if that fails. Files are always served through Vizier's own authenticated endpoints (with range requests), never through public or pre-signed bucket links.

### Topology and backups

Run **one** Vizier process per database and bucket. Several processes on one deployment are not supported yet (that is [Vizier Fleet](https://github.com/vizier-lab/vizier/issues/138)), even though the storage layer is safe for concurrent writers.

Back up the database with your usual PostgreSQL tooling (`pg_dump`, managed snapshots), the bucket with versioning or replication, and the encryption key separately from both.

### Moving an existing workspace

```sh
vizier shutdown   # the source must not be running
export VIZIER_DATABASE_URL=… VIZIER_S3_BUCKET=… VIZIER_S3_ACCESS_KEY_ID=… VIZIER_S3_SECRET_ACCESS_KEY=…
vizier storage migrate --workspace /srv/vizier --storage postgres --objects s3 > report.json
```

The command copies every table, the memory vectors and every file, verifying each file's size and SHA-256, and prints a JSON report with counts only (no content, no secrets). It reads the source and never changes it. An interrupted run resumes where it stopped; a second run for the same source is refused unless you pass `--reset-destination`. Use the same encryption key on the new deployment. To roll back before the new deployment has accepted any writes, start the untouched source workspace again.

Exit codes: `0` done, `1` failed (rerun to resume), `2` refused (source running, destination not empty, or already migrated).

The `indexer` key that older docs mentioned at this level no longer exists — vector indexing is a per-agent setting (see [Tools & Embedding](./tools-embedding.md#indexer-indexer)).

### Startup migrations

On SQLite, `VizierDependencies::new` runs these once, guarded by markers in the `state` table (a PostgreSQL database is always created at the current layout, so it needs none of them):

1. Flat memory files → bundles (pre-bundle layouts)
2. `filesystem` backend → SQLite (if the config still says `filesystem`)
3. Seed users → `superadmin` system role
4. Provider credentials saved under pre-catalogue names → catalogue ids (`gemini` → `google`, …), once
5. Per-agent MCP / shell config backfill (from the old global config)
6. Legacy identity backfill — the source the spawn-time `core/soul` seed reads (retired before 1.0.0, #82)

## Shell

Shell access is the **Shell addon**, added per agent from its Addons page. Each agent owns its own shell instance; there is no global shell. Without the addon, the agent has no `shell_exec` tool.

The tool runs `sh -c "<commands>"` (PowerShell on a Windows host) and returns stdout. Each call is bounded by the agent's `tools.timeout` (default `30m`).

### Local

Install the addon with:

```json
{
  "kind": "shell",
  "name": "Shell",
  "settings": {
    "path": "/home/me/project",
    "env": "RUST_LOG=debug\nTZ=UTC"
  }
}
```

| Setting | Required | Description |
|---------|----------|-------------|
| `path` | yes | Working directory for every command |
| `env` | no | Extra environment variables, one `KEY=value` per line (stored as a secret and never returned by the API) |

The command runs with the same OS user as the Vizier process — treat a local shell as fully trusted.

### Docker

Not offered yet. Docker shells are being rewritten; the Shell addon is local-only for now.

### Managing at runtime

- **WebUI**: agent → **Addons** → Shell → Manage
- **API**: `POST /api/v1/agents/{id}/integrations` to add it, or `PUT /api/v1/integrations/{id}` with new settings

Changing an addon's settings takes effect from the agent's next turn; it does not restart the agent.
