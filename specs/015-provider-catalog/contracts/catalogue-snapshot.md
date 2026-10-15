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
assets/catalogue/                      # committed to git
├── meta.json        {"schema": 1, "source_url": "https://models.dev/api.json",
│                     "fetched_at": "2026-10-15T09:12:00Z", "etag": "\"a819…\"",
│                     "providers": 230, "models": 8870, "no_logo": ["…"]}
├── api.json         trimmed models.dev JSON, plain text: keys sorted, one model per line
└── logos/{id}.svg   one per provider id or lab id with its own logo
```

**Why plain JSON is committed**:
- Git delta-compresses it, so each refresh adds only what changed.
- A refresh diff is reviewable (e.g. "+ claude-opus-5 …", "status: deprecated").

**Build**: `build.rs`, which already exists to build the WebUI, gzips `api.json` into `$OUT_DIR/catalogue.json.gz` with `flate2`. `flate2` is already in the dependency tree and is added as a build-dependency only. The binary embeds that file with `include_bytes!`, and `meta.json` and `logos/` with `rust-embed`, for about 650 KB in all. A missing or unparsable `assets/catalogue/api.json` is a build error, with a message pointing at `just catalogue-update`. `cargo build` never accesses the network.

**Packaging**: `Cargo.toml` `include` gains `"/assets/catalogue/**/*"`, so the crates.io package builds; without it, `cargo install vizier` would fail. `publish.yml`'s existing `cargo package --list` step fails if `assets/catalogue/api.json` is missing from the list.

### When the seed is downloaded

**Once per release**, in the release workflow (`.github/workflows/release.yml`, job `prepare-release`), between the version bump and the changelog:

```
prepare-release:  bump Cargo.toml → catalogue_update.py → git-cliff → commit + tag → push
build ×4, cargo publish, docker:  check out the tag → all ship the same snapshot
```

The new step:
- runs `python3 -I scripts/catalogue_update.py --keep-on-failure`;
- then `git add assets/catalogue`, so the snapshot rides in the existing `chore: bump version to X` commit, and the tag contains it.

**Dry runs** fetch and validate, but push nothing, like the rest of the dry run.

**Failure** (owner decision, 2026-10-15): if models.dev is unreachable, or the response fails validation, the step **keeps the committed snapshot** and exits 0. It writes a `::warning::` and a run-summary line giving the snapshot's `fetched_at` age. A catalogue outage never blocks a release.

**Between releases**: there is no scheduled job (owner decision). Developers run `just catalogue-update`, a wrapper around the same script, whenever they want. The release runner calls the script directly, since it doesn't need `just`. Normal `cargo build`, `just install` and CI never fetch.

### `scripts/catalogue_update.py`

Standard library only, run with `python3 -I`:

1. GET `api.json`.
2. Validate it, with the same rules as the runtime refresh, comparing against the currently committed snapshot.
3. Trim it to the kept fields.
4. Collect the logo ids: every provider id, plus every `canonical_model_id` prefix.
5. GET each logo, eight at a time. A logo whose bytes equal models.dev's fallback (the response for an unknown id) is recorded in `no_logo` instead of being saved. Logos that fail to download keep their committed file.
6. Write `api.json` (sorted, one model per line), `meta.json` and `logos/`. Remove logo files for ids that no longer exist. Print the counts and the size.

| Flag | Effect |
|---|---|
| `--keep-on-failure` | on any fetch or validation error: print a warning, leave the files untouched, exit 0 (release use). Without it the script exits 1 (local use) |

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
