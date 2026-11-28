# Data Model: Agent Apps

All tables live in the main SQLite database. They are created by `storage/sqlite/app.rs::init_app_schema` and only when the `addon-apps` feature is compiled in. Times are epoch milliseconds unless noted. Ids are prefixed base62: `ap_` for apps, `as_` for schedules, `af_` for files.

## Tables

### `app`

| Column | Type | Notes |
|---|---|---|
| `id` | TEXT PK | `ap_` + 22 base62 |
| `owner_user_id` | TEXT NOT NULL | The only person who may see or use the app (FR-002) |
| `name` | TEXT NOT NULL | `Untitled app` while a draft |
| `icon` | TEXT NULL | An `IconName`; NULL → monogram |
| `description` | TEXT NOT NULL DEFAULT '' | |
| `current_version` | INTEGER NOT NULL DEFAULT 0 | 0 = **draft** (no version yet) |
| `latest_version` | INTEGER NOT NULL DEFAULT 0 | Rollback moves `current_version` only |
| `driver_agent_id` | TEXT NULL | NULL = no driver (FR-026) |
| `thread_topic` | TEXT NOT NULL | The current thread's topic; a new value starts a new thread |
| `badge` | INTEGER NULL | The tile count (FR-032) |
| `db_bytes` / `files_bytes` | INTEGER NOT NULL DEFAULT 0 | Cached sizes for limits and the owner view |
| `created_at`, `updated_at` | INTEGER | |

Index: `(owner_user_id)`, `(driver_agent_id)`.

### `app_version`

| Column | Type | Notes |
|---|---|---|
| `app_id` | TEXT | FK → `app.id` ON DELETE CASCADE |
| `version` | INTEGER | PK `(app_id, version)`, from 1 |
| `page_sha256` | TEXT NOT NULL | Blob `apps/{app_id}/pages/{sha256}.html` |
| `page_size` | INTEGER NOT NULL | |
| `manifest` | TEXT NOT NULL | JSON, see [contracts/manifest.md](contracts/manifest.md) |
| `base_version` | INTEGER NULL | NULL for v1 |
| `origin` | TEXT NOT NULL | `driver:{agent_id}` or `rollback:{from}` (a rollback adds no row; this is reserved for imports) |
| `verification` | TEXT NOT NULL | `pending`, `ok`, `problems:{json}`, `unverified:{reason}` (as spec 063) |
| `history_uid` | TEXT NULL | The step entry in the thread that made it |
| `created_at` | INTEGER | |

### `app_approval`

The owner's consent for the page to call a function under one driver (FR-029).

| Column | Type | Notes |
|---|---|---|
| `app_id`, `driver_agent_id`, `function` | TEXT | PK of all three |
| `sensitive` | INTEGER NOT NULL | 1 when the function's derived calls use secrets or tools with outside effects; such approvals are not carried to a new driver |
| `granted_at` | INTEGER | |

Rule: a page call to `f` is allowed only when `f` is in the current manifest's `page_callable` **and** a row `(app, current driver, f)` exists. When the driver changes, rows with `sensitive = 0` are copied to the new driver and rows with `sensitive = 1` are not.

### `app_schedule` / `app_schedule_run`

`app_schedule` is rebuilt from the current version's manifest whenever `current_version` changes, keyed `(app_id, name)`, with `cron`, `kind` (`run` | `ask`), `target`, `episode` (NULL or an open failure episode id) and `last_run_at`.

`app_schedule_run(id, app_id, schedule, driver_agent_id NULL, started_at, finished_at NULL, state, reason NULL)`:

```text
running ──► ok
        ├─► failed     (opens or continues an episode → one notification per episode)
        └─► skipped    (no_driver | missing:{gaps} | app_removed)
```

A `running` row is the overlap lock. Startup marks leftover `running` rows `failed` with the reason `interrupted`.

### `rail_order`

| Column | Type | Notes |
|---|---|---|
| `user_id` | TEXT PK | |
| `items` | TEXT NOT NULL | JSON array of `{"kind":"agent","id":…}` / `{"kind":"app","id":…}` |
| `updated_at` | INTEGER | |

Read rule (pure, `lib/apps/rail.ts` and `apps::rail::arrange`): known items keep the stored order. Items the person can see but that are not stored are appended (agents first, then apps, each in creation order). Stored items that are gone are dropped.

## Files on disk

| Path | Content |
|---|---|
| `{workspace}/apps/{app_id}/app.sqlite` (+ `-wal`, `-shm`) | The app database. Opened per use with WAL and `busy_timeout`. `PRAGMA trusted_schema=OFF` and `PRAGMA cell_size_check=ON`. No `ATTACH` and no `load_extension` (authorizer denies them) |
| blob `apps/{app_id}/pages/{sha256}.html` | Page HTML, content-addressed |
| blob `apps/{app_id}/files/{af_id}` | App files; the name and type live in the app database's `_vizier_files` table |

Inside each app database, Vizier keeps two tables that page and driver SQL cannot write (the authorizer refuses writes to `_vizier_*`):

- `_vizier_migrations(seq INTEGER PK, version INTEGER, sha256 TEXT, applied_at INTEGER)`
- `_vizier_files(id TEXT PK, name TEXT, media_type TEXT, size INTEGER, origin TEXT, created_at INTEGER)`

## Entities in the engine

- **`App`**: the `app` row plus its derived state:
  - `Draft` when `current_version = 0`;
  - `NoDriver` when `driver_agent_id` is NULL or the owner may no longer chat with that agent;
  - `Ready` otherwise.
- **`AppVersion`**: a row plus its parsed `Manifest`.
- **`Manifest`**: see the contract. Validation reports every problem at once, the custom-tool way (`status` results, never `Err`, in tools).
- **`AppTurnContext`**: the app, its current version's manifest, its tables with row counts, the requirement `Report` for this driver, and `tab_open: bool`.
- **`Report`**: `Ready | Missing(Vec<Gap{requirement, reason, fix}>)`. Pure.
- **`AppOrigin { app_id, version }`**: set only by the `/apps/{id}/chat` route on a `Chat` request (research D12).
- **`AppEvent`**: `DataChanged{app, tables}`, `VersionSaved{app, version}`, `DriverChanged{app, driver}`, `BadgeChanged{app, badge}`. Published on `app.{id}.events`.

## State transitions

```text
              New app (+)                   first saved version
  (none) ───────────────────► Draft ───────────────────────────► App
                                 │ left with no message
                                 └──► deleted

  App:  driver set ◄──► no driver   (agent deleted, access lost, or the owner clears it)
        current_version: any saved version (rollback / forward); latest_version only grows
        removed ──► rows, blobs and database file deleted; threads stay with their agents
```

## Validation rules (from the spec)

- **Owner**: every route resolves the principal and requires `app.owner_user_id == principal.user`; anything else is `404` (FR-002).
- **Driver**: only an agent the owner may chat with (`access::can_view_agent` + the chat right). Re-checked on every turn and every page function call (FR-025, FR-026).
- **First version**: must set `name` and `icon` (research D14).
- **Migrations**: append-only; the SQL of an applied migration must hash the same in every later manifest; each new migration runs in the transaction that saves the version (FR-018a, edge case).
- **Page SQL**: parameterised. DDL, `ATTACH`, `PRAGMA` writes and `_vizier_*` writes are refused. Limited to 2 s and 5,000 rows. A write that would cross the database size limit is refused (FR-019).
- **Chat sends from the page**: only within the gesture window, at most 1 per second and 20 per minute per app, and only with a driver (FR-021a).
- **Manifest tools**: at most 32, client-tool name rules; the server keeps only declarations present in the current manifest (FR-033).
