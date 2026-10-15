# Contract: Catalogue Snapshot Format and Refresh

## Archive layout

The same layout is used for the bundled snapshot and for refreshed copies:

```
catalogue.zip
├── meta.json        {"schema": 1, "source_url": "https://models.dev/api.json",
│                     "fetched_at": "2026-10-15T09:12:00Z",
│                     "providers": 230, "models": 8870,
│                     "no_logo": ["…ids whose logo is the models.dev fallback…"]}
├── api.json         trimmed models.dev JSON (fields below), compact
└── logos/{id}.svg   one per provider id or lab id that has its own logo
```

**Where each copy lives**:

- **Bundled**: `assets/catalogue/catalogue.zip`, committed to git and embedded with `include_bytes!`.
- **Refreshed**: `{workspace}/.runtime/catalogue/catalogue.zip`. It is written to `catalogue.zip.tmp-<pid>`, flushed to disk, then renamed over the old file.

**At startup**: load the refreshed copy if it exists, parses, has `schema == 1` and has a `fetched_at` newer than the bundled one. Otherwise use the bundled copy. A corrupt refreshed copy is logged at `warn` and then ignored.

## Kept fields (everything else is dropped)

**Provider**: `id`, `name`, `env`, `npm`, `api`, `doc`, `models`.

**Model**: `id`, `name`, `family`, `attachment`, `reasoning`, `tool_call`, `temperature`, `knowledge`, `release_date`, `last_updated`, `modalities`, `limit`, `cost`, `status`, `canonical_model_id`, `provider` (the per-model `npm`/`api`/`shape` override).

**Parsing**: unknown fields are ignored. A missing optional field means unknown. Parsing never fails on one bad model: that model is skipped, logged at `debug`, and the rest of the catalogue still loads.

## Producing the bundled snapshot

The recipe `just catalogue-update` runs `scripts/catalogue_update.py` (standard library only, `python3 -I`) with these steps:

1. GET `https://models.dev/api.json`.
2. Trim it to the kept fields.
3. Collect the logo ids: every provider id, plus every `canonical_model_id` prefix.
4. GET `https://models.dev/logos/{id}.svg` for each. Record as `no_logo` any logo whose bytes equal those of `logos/__none__.svg`, which is the fallback models.dev serves for an unknown id.
5. Write the zip, then print the counts and the size.

The script is run at release time, and its result is committed as a normal file change (`chore: refresh model catalogue`). `cargo build` never touches the network.

## Runtime refresh

This is the same pipeline as the script, run in Rust inside the refresher (R9). Logos are fetched with a concurrency of at most 8 and a timeout of 10 seconds each. Logos that fail to download keep their previous bytes.

A new archive is **accepted only if** all of these hold:

1. `api.json` parses, and at least one provider has a supported dialect.
2. The provider count is at least 50% of the current catalogue's.
3. Every catalogue provider id that a stored agent, credential row or media setting uses is still present.

Otherwise the previous archive stays, and the reason is stored for `GET /catalogue`'s `refresh.last_error`.

**Schedule**: one attempt every `refresh_interval`, starting 60 seconds after startup. Failures don't retry before the next interval; an admin can trigger a refresh by hand. With `refresh: false`, no request is ever made (FR-004), and `POST /catalogue/refresh` returns `409`.

## Logos in the WebUI

The WebUI always renders logos as `<img src="/api/v1/catalogue/logos/{id}.svg" alt="{name}">`, never inline.

**Choosing the image**:

- **Provider**: its own logo; otherwise initials in a neutral badge.
- **Model**: its lab's logo (`canonical_model_id` prefix); otherwise its provider's logo; otherwise initials.
- **Failed load** (`onError`): the initials badge, at the same box size, so the layout never shifts.
- **Dark theme**: when the theme is dark, `currentColor` logos are recoloured with a CSS `filter: invert(1)`.
