# Contract: Catalogue Seeding, Cache and Lazy Refresh

## Overview

| Layer | Where | Written by | Purpose |
|---|---|---|---|
| **Seed** | `assets/catalogue/` in the repo, embedded in the binary | `just catalogue-update` at release time | always available: first start, offline, air-gapped |
| **Cache** | SQLite tables `catalogue_snapshot`, `catalogue_logo` | the lazy refresher | newer data that survives restarts, offline included |
| **Memory** | `Catalogue` on `VizierDependencies` | startup, and each successful refresh | every query (pickers, `resolve`) reads only this |

`cargo build` never accesses the network.

## Seed (build time)

```
assets/catalogue/
├── meta.json        {"schema": 1, "source_url": "https://models.dev/api.json",
│                     "fetched_at": "2026-10-15T09:12:00Z", "etag": "\"a819…\"",
│                     "providers": 230, "models": 8870, "no_logo": ["…"]}
├── api.json.gz      trimmed models.dev JSON (fields below), gzip
└── logos/{id}.svg   one per provider id or lab id with its own logo
```

The files are embedded with `rust-embed`, the same way `webui/build/client/` is. Total size is about 650 KB.

`just catalogue-update` runs `scripts/catalogue_update.py` (standard library only, `python3 -I`):

1. GET `api.json`.
2. Trim it to the kept fields.
3. Collect the logo ids: every provider id, plus every `canonical_model_id` prefix.
4. GET each logo. A logo whose bytes equal models.dev's fallback (the response for an unknown id) is recorded in `no_logo` instead of being saved.
5. Write the files and print the counts.

The release workflow runs it before tagging and commits the result as `chore: refresh model catalogue`. Developers can run it any time.

## Kept fields

**Provider**: `id`, `name`, `env`, `npm`, `api`, `doc`, `models`.

**Model**: `id`, `name`, `family`, `attachment`, `reasoning`, `tool_call`, `temperature`, `knowledge`, `release_date`, `last_updated`, `modalities`, `limit`, `cost`, `status`, `canonical_model_id`, `provider` (the per-model `npm`/`api`/`shape` override).

**Parsing**: unknown fields are ignored. A missing optional field means unknown. One bad model is skipped and logged at `debug`; it never fails the whole catalogue.

## Cache (SQLite)

```sql
CREATE TABLE IF NOT EXISTS catalogue_snapshot (
    id          INTEGER PRIMARY KEY CHECK (id = 1),   -- single row
    schema      INTEGER NOT NULL,
    source_url  TEXT NOT NULL,
    etag        TEXT,
    fetched_at  INTEGER NOT NULL,   -- ms; when this data was downloaded (200)
    checked_at  INTEGER NOT NULL,   -- ms; last successful check (200 or 304)
    last_error  TEXT,               -- last failed check, cleared on success
    failed_at   INTEGER,            -- ms; for the 1h backoff
    data        BLOB NOT NULL       -- gzip of the trimmed api.json
);
CREATE TABLE IF NOT EXISTS catalogue_logo (
    id          TEXT PRIMARY KEY,   -- provider or lab id
    svg         BLOB,               -- NULL = known to have no logo
    fetched_at  INTEGER NOT NULL
);
```

**Why blobs, not one row per model**:
- Every query runs against memory.
- Normalised tables would mean writing about 8,900 rows on each refresh, plus a schema migration whenever models.dev adds a field.

Writes happen in one transaction, so two processes sharing a workspace can't corrupt the cache: the last writer wins with valid data.

## Startup

1. Parse the seed.
2. If `catalogue_snapshot` exists, has `schema == 1`, parses, and its `fetched_at` is newer than the seed's, use it instead. A corrupt cache row is logged at `warn` and ignored; the seed is used.
3. Build `CatalogueData` in memory (target: under 200 ms).

Startup never makes a network request.

## Lazy refresh

**Triggers**: only requests from people.
- **Yes**: every `/catalogue/*` read endpoint, `GET /providers`, `GET /providers/custom`, and `POST /catalogue/refresh` (which ignores freshness).
- **No**: agent builds, restarts, turns, the scheduler and the dream cycle. They read memory only (owner decision 2026-10-15). A headless install never contacts models.dev.

**On a trigger**:
1. Stop if `catalogue.refresh` is false.
2. Stop if the data was checked within the freshness window (`now - checked_at < catalogue.ttl`, default 24h).
3. Stop if the last check failed less than an hour ago (`failed_at`).
4. Stop if a check is already running (single-flight).
5. Otherwise, spawn **one** background task and return the current data to the caller immediately (stale-while-revalidate).

**The task**: `GET {catalogue.url}` with `If-None-Match: <etag>` and `Accept-Encoding: gzip`, and a timeout of 30 seconds.

| Response | Action |
|---|---|
| `304` | update `checked_at`; memory unchanged |
| `200` and validation passes | trim; in one transaction write `data`, `etag`, `fetched_at`, `checked_at` and clear `last_error`; swap memory |
| `200` and validation fails, or any error | store `last_error` and `failed_at`; memory and cache unchanged |

**Validation**: all of these must hold.
1. The body parses, and at least one provider has a supported dialect.
2. The provider count is at least 50% of the current catalogue's.
3. Every catalogue provider id that a stored agent, credential row or media setting uses is still present.

**Swapping memory**: readers clone `Arc<CatalogueData>`, so they never wait. Agents keep the data they were built with until their next build.

**`POST /catalogue/refresh`** (admin) runs the same task but waits for it, up to 30 seconds, and returns the outcome. With `catalogue.refresh: false` it returns `409`.

## Logos

**Lookup order** for `GET /api/v1/catalogue/logos/{id}.svg`:

1. **`catalogue_logo` row**: serve its `svg`, or `404` if `svg` is NULL.
2. **Seed `logos/{id}.svg`**: serve it.
3. **Seed `no_logo` contains `id`**: `404`.
4. **Otherwise** (a provider or lab added after the build), when refresh is enabled: fetch `https://models.dev/logos/{id}.svg` in the background, store the result (`svg`, or NULL if it is the fallback image), and return `404` for now. The next view shows the logo.

Logos are served with `Content-Type: image/svg+xml`, `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'`, `X-Content-Type-Options: nosniff` and `Cache-Control: public, max-age=86400`. The browser never contacts models.dev.

**WebUI rendering**: always `<img src=… alt=…>`, never inline.

- **Model**: its lab's logo (`canonical_model_id` prefix); otherwise its provider's logo; otherwise initials.
- **Failed load** (`onError`): an initials badge, at the same box size.
- **Dark theme**: `currentColor` logos are recoloured with `filter: invert(1)`.

## Settings

| Key | Env | Default |
|---|---|---|
| `vizier.catalogue.refresh` | `VIZIER_CATALOGUE_REFRESH` | `true` |
| `vizier.catalogue.ttl` | `VIZIER_CATALOGUE_TTL` | `24h` |
| `vizier.catalogue.url` | — | `https://models.dev/api.json`; logos come from the same origin's `/logos/` |
