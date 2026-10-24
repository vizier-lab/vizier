# Data Model: OpenRouter Media Provider

**Feature**: `028-openrouter-media-provider` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

This model covers the provider enum additions, the per-capability media model listing, the persisted snapshot, and the refresh state. OpenRouter credentials, agent settings and generated artifacts are existing entities; only the fields that change are listed.

## 1. Capability

`MediaCapability` is the one axis discovery varies on. It is also the storage key and the API query value.

| Value | Agent setting | Provider enum | Live source (2026-10-24) |
|---|---|---|---|
| `speech` | `tools.tts_settings` | `TtsProvider` | `GET /api/v1/models?output_modalities=speech` |
| `transcription` | `tools.stt_settings` | `SttProvider` | `GET /api/v1/models?output_modalities=transcription` |
| `image` | `tools.image_gen_settings` | `ImageGenProvider` | `GET /api/v1/images/models` |

Rules:

- Exactly these three values are accepted by the API and stored in the table; anything else is a 400 / ignored row.
- `default_model()` delegates to the matching provider enum's `Openrouter` value, so the factory and the catalogue cannot drift:
  - `speech` → `TtsProvider::Openrouter.default_model()`
  - `transcription` → `SttProvider::Openrouter.default_model()`
  - `image` → `ImageGenProvider::Openrouter.default_model()`
- `default_voice()` is `Some(TtsProvider::Openrouter.default_voice())` for `speech`, `None` otherwise.
- A display label (`Text to speech`, `Speech to text`, `Image generation`) is a WebUI concern, not stored.

### Documented defaults

| Capability | Default model | Default voice | Why |
|---|---|---|---|
| `speech` | `hexgrad/kokoro-82m` | `af_alloy` | Live, open-weight, MP3-capable, stable voice names. Replaces the retired `openai/gpt-4o-mini-tts-2025-12-15` / `alloy`. |
| `transcription` | `openai/whisper-large-v3` | — | Live; existing documented candidate. |
| `image` | `bytedance-seed/seedream-4.5` | — | Live; existing documented candidate. |

The voice is part of the default pair: changing the speech model means re-checking that the voice exists in its `supported_voices` and updating `TtsProvider::default_voice()`, the WebUI map and the docs together.

## 2. Media model listing

### `MediaModel` (domain, seed and cache payload)

| Field | Type | Rules |
|---|---|---|
| `id` | `String` | Required, non-empty after trim, unique within the capability. Stored exactly as OpenRouter returns it (case preserved). |
| `name` | `String` | Readable name; falls back to `id` when missing/empty. |
| `voices` | `Vec<String>` | Speech only; empty for transcription/image. Order preserved; duplicates removed. |
| `aspect_ratios` | `Vec<String>` | Image only; from the model's `supported_parameters.aspect_ratio.values` when it is an enum; otherwise empty. |
| `resolutions` | `Vec<String>` | Image only; from `supported_parameters.resolution.values` when it is an enum; otherwise empty. |

`default` and `listed` are **not** stored: they are derived when a listing is built, because the default is a provider policy, not provider data. The API also returns a top-level `default_voice: Option<String>` (`Some(TtsProvider::Openrouter.default_voice())` for `speech`, `None` otherwise); it is the documented voice, not per-model data, and lets the voice field show the server's value once the list loads.

### `MediaList` (in memory, one per capability)

| Field | Type | Meaning |
|---|---|---|
| `capability` | `MediaCapability` | Which list this is. |
| `models` | `Vec<MediaModel>` | Provider order after parsing/validation. |
| `source` | `CatalogueSource` (`Bundled` / `Cached`) | Where the current bytes came from. |
| `fetched_at` | `DateTime<Utc>` | When the current bytes were successfully fetched (seed timestamp or refresh time). |
| `checked_at` | `Option<DateTime<Utc>>` | Last successful check; `None` for the seed until a refresh. TTL uses `checked_at.unwrap_or(fetched_at)`. |

### `MediaModelSummary` (API only)

Adds two fields to `MediaModel`:

| Field | Type | Meaning |
|---|---|---|
| `default` | `bool` | True for the documented default model id. |
| `listed` | `bool` | True when the entry came from the seed/cache fetch; false only for an injected default that the current list no longer contains. |

Default injection rules:

1. If the documented default id is in `models`, set `default: true`, `listed: true` on that entry.
2. If it is absent, prepend an entry with that id, `name = id`, empty voices/ratios/resolutions, `default: true`, `listed: false`. This keeps the effective value visible, satisfies FR-008/FR-023, and never silently substitutes a different model.
3. All other entries have `default: false`, `listed: true`.
4. Do not duplicate a default that is already present; do not reorder the remaining entries.

## 3. Persisted snapshot

### `MediaCatalogueSnapshotRow`

| Field | Type | Meaning |
|---|---|---|
| `capability` | `String` | `speech` / `transcription` / `image`; primary key. |
| `schema` | `i64` | Payload version, currently `1`. A mismatch ignores the row. |
| `fetched_at` | `DateTime<Utc>` | Successful fetch time. |
| `checked_at` | `DateTime<Utc>` | Last successful check (equal to `fetched_at` on a new row; a later successful refresh moves it while replacing the data). |
| `last_error` | `Option<String>` | Reason for the last failed check, for diagnostics and the restored notice. |
| `failed_at` | `Option<DateTime<Utc>>` | When that failure happened; drives the 1 h backoff. |
| `data` | `Vec<u8>` | gzip of `serde_json::to_vec(&Vec<MediaModel>)`. |

### SQLite DDL

Added in `SqliteStorage::init_schema` next to `catalogue_snapshot`:

```sql
CREATE TABLE IF NOT EXISTS media_catalogue_snapshot (
    capability  TEXT PRIMARY KEY CHECK (capability IN ('speech', 'transcription', 'image')),
    schema      INTEGER NOT NULL,
    fetched_at  INTEGER NOT NULL,
    checked_at  INTEGER NOT NULL,
    last_error  TEXT,
    failed_at   INTEGER,
    data        BLOB NOT NULL
);
```

Semantics:

- `get_media_snapshot(capability)` → row or `None`.
- `put_media_snapshot(row)` → `INSERT ... ON CONFLICT(capability) DO UPDATE`, clears `last_error`/`failed_at` (a success supersedes a failure), one transaction.
- `record_media_failure(capability, error, at)` → set `last_error`/`failed_at`, leave data and `checked_at` untouched.
- All timestamps are stored as milliseconds, matching `catalogue_snapshot`.
- Deleting an agent does not delete a media snapshot: it is provider-wide, not agent-scoped.

## 4. Seed

`assets/catalogue/openrouter_media.json`:

```json
{
  "schema": 1,
  "fetched_at": "2026-10-24T00:00:00Z",
  "speech": [
    { "id": "hexgrad/kokoro-82m", "name": "hexgrad: Kokoro 82M",
      "voices": ["af_alloy", "af_aoede"] }
  ],
  "transcription": [
    { "id": "openai/whisper-large-v3", "name": "OpenAI: Whisper Large V3" }
  ],
  "image": [
    { "id": "bytedance-seed/seedream-4.5", "name": "ByteDance Seed: Seedream 4.5",
      "aspect_ratios": ["1:1", "16:9"], "resolutions": ["1K", "2K", "4K"] }
  ]
}
```

Rules:

- `schema` must be `1`; an unknown schema means "no usable seed" (empty lists, logged), not a crash.
- `fetched_at` is required and must parse as RFC 3339; it is compared with the cached row.
- Each capability key may be missing/empty; the typed model id still works.
- Unknown fields are ignored; missing `name` falls back to `id`; missing arrays default to empty.
- `build.rs` only validates JSON syntax (a malformed seed should fail the build early, but the runtime still tolerates a logically empty seed).

## 5. Refresh state

Per capability, in memory:

| Field | Type | Meaning |
|---|---|---|
| `in_progress` | `AtomicBool` | One check at a time per capability. |
| `failure` | `Mutex<Option<(String, DateTime<Utc>)>>` | Last failure reason/time; drives the 1 h backoff and the API notice. Restored from the snapshot row at startup. |

`MediaStatus` (API):

| Field | Type | Source |
|---|---|---|
| `enabled` | `bool` | `config.catalogue.refresh` |
| `ttl` | `String` | `config.catalogue.ttl.to_string()` (same as the chat catalogue) |
| `in_progress` | `bool` | `AtomicBool` |
| `last_error` | `Option<String>` | in-memory failure (restored at startup) |
| `failed_at` | `Option<DateTime<Utc>>` | in-memory failure time |

## 6. State transitions

```text
fresh install ──► seed (source=bundled, fetched_at=seed, checked_at=None)
                     │
read within TTL ─────┘ (no network)
                     │
read after TTL ──► background check (source stays, in_progress=true)
                     │
          success ───┼──► validate ──► put snapshot ──► swap (source=cached, checked_at=now,
                     │                                      failure=null)
          failure ───┴──► keep list ──► record last_error/failed_at ──► 1 h backoff

startup ──► seed, then per capability: cached row if schema=1 and fetched_at>seed.fetched_at,
            else seed; restore last_error/failed_at from the row. Never network.

manual refresh ──► disabled? 409
                    else wait (≤30 s) for an in-flight check, or run one
                    success → 200 + listing
                    failure → 502 + reason (previous list still served)

default id absent from current list ──► API prepends it (listed=false, default=true)
selected id absent from refreshed list ──► form value is untouched; picker shows it as typed
credential missing ──► list still works; a media call fails with the OpenRouter Settings hint
```

## 7. Provider changes

### Enums

| Enum | New variant | `credentials()` | `default_model()` | Other |
|---|---|---|---|---|
| `TtsProvider` | already has `Openrouter` | `OPENROUTER` | `hexgrad/kokoro-82m` | `default_voice()` returns `af_alloy` for OpenRouter; OpenAI keeps `alloy`. |
| `SttProvider` | `Openrouter` | `OPENROUTER` | `openai/whisper-large-v3` | Uses the existing `OpenAiStt` arm. |
| `ImageGenProvider` | `Openrouter` | `OPENROUTER` | `bytedance-seed/seedream-4.5` | Uses `OpenAiImages` with `ImageApi::OpenRouter`. |

No agent-config migration: serde adds variants, and existing values are unchanged. The WebUI lists are the only discovery surface that needs the new value.

### `TtsProvider::default_model()`

New method; the factory stops matching on provider for the model. Values:

| Provider | Default model |
|---|---|
| `openai` | `tts-1` |
| `openrouter` | `hexgrad/kokoro-82m` |
| `elevenlabs` | `eleven_multilingual_v2` |
| `xai` | `grok-2-tts` |
| `hyperbolic` | `Melo-TTS` |
| `kokoro` | `kokoro-en-v0_19` |

### Image artifact

`VizierImageGen::generate` returns `(bytes, mime)`. When `image_generate` has no explicit filename:

| MIME | Extension |
|---|---|
| `image/png` | `.png` |
| `image/jpeg`, `image/jpg` | `.jpg` |
| `image/webp` | `.webp` |
| `image/svg+xml` | `.svg` |
| anything else | `.png` (existing fallback) |

The session-file row always stores the returned MIME, so channel renderers use the MIME, not the extension. An explicit filename is respected even if its extension disagrees.

## 8. Validation rules (refresh and seed)

A fetched list is accepted only when:

1. The response is JSON with a `data` array (speech/transcription) or an array for the image endpoint, per parser.
2. It yields at least one entry with a non-empty `id` after trimming.
3. Every accepted id is unique within the capability (first wins).
4. The list has at most 2,000 entries (defensive cap; live lists are ~60).
5. Capability-specific arrays are strings only; non-strings are dropped, not fatal.
6. Missing `name` becomes the id.

A failed check never replaces a usable list. A malformed seed is logged and yields empty lists; the typed fallback and documented default still apply.

## 9. Relationships

- An **agent media setting** selects a provider; when that provider is OpenRouter, the WebUI reads the matching **media listing**.
- A **media listing** is keyed by capability, not by agent or provider row; all OpenRouter agents share it.
- A **snapshot row** is one capability's last successful listing plus its last failure.
- A **default** is a property of the OpenRouter integration (implemented by the provider enum), referenced by the listing and used by the factory when the setting is empty.
- A **generated artifact** is an existing session file; the only change is choosing its default extension from the returned MIME.
