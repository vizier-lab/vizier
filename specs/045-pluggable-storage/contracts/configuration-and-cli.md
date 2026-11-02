# Contract: Configuration and CLI

## YAML

```yaml
vizier:
  storage:
    type: postgres            # sqlite (default) | postgres
    url_env: VIZIER_DATABASE_URL   # default; the URL itself is never in YAML
    pool_size: 8
    ca_file: /run/secrets/pg-ca.pem   # optional; system roots otherwise
  objects:
    type: s3                  # local (default) | s3
    bucket: vizier
    region: us-east-1
    endpoint: https://minio.internal:9000   # optional
    path_style: true          # default true when endpoint is set
    prefix: prod              # optional
```

Existing configs (`type: sqlite`, no `objects`) are unchanged. `type: filesystem` keeps its current legacy migration behavior.

## CLI and environment

| Flag | Env | Values |
|---|---|---|
| `--storage` | `VIZIER_STORAGE` | `sqlite` (default), `postgres` |
| `--objects` | `VIZIER_OBJECTS` | `local` (default), `s3` |
| — | `VIZIER_DATABASE_URL` | `postgres://…?sslmode=require` |
| — | `VIZIER_S3_BUCKET`, `VIZIER_S3_REGION`, `VIZIER_S3_ENDPOINT`, `VIZIER_S3_PATH_STYLE`, `VIZIER_S3_PREFIX` | override YAML |
| — | `VIZIER_S3_ACCESS_KEY_ID`, `VIZIER_S3_SECRET_ACCESS_KEY`, `VIZIER_S3_SESSION_TOKEN` | credentials (fallback `AWS_*`) |

Credentials and URLs are never accepted as CLI arguments and are redacted from `Debug` and serialized config.

## Startup validation (FR-004)

Refuse to start, exit code 1, with one of these stable messages (no URL, user, password or key):

- `storage: VIZIER_DATABASE_URL is not set`
- `storage: cannot connect to PostgreSQL (<sqlstate or io kind>)`
- `storage: PostgreSQL is missing the "vector" extension (CREATE EXTENSION vector)`
- `storage: database schema version <n> is newer than this binary (<m>)`
- `objects: S3 bucket "<bucket>" is not reachable (<http status or io kind>)`
- `objects: S3 credentials are missing`

## Desktop

`app/src-tauri` always builds `StorageConfig::Sqlite` + `ObjectsConfig::Local` and ignores these environment variables.

## Migration command

```text
vizier storage migrate --workspace <dir> [--reset-destination] [--dry-run]
```

Destination comes from the same `storage`/`objects` configuration and environment (must be `postgres`, and `s3` or `local`). Progress lines go to stderr; the final report is JSON on stdout:

```json
{"migration_id":"…","tables":{"agent_config":{"rows":3}, "…": {}},"vectors":{"document_index_d384":{"rows":1200}},
 "blobs":{"count":42,"bytes":10485760,"verified":42},"duration_ms":51234}
```

Exit codes: 0 done, 1 failed (resumable), 2 refused (source running, destination not empty or already migrated).
