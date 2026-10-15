# Contract: HTTP API (`/api/v1`)

Every response uses the existing `APIResponse<T>` envelope (`{status, message, data}`). Auth middleware is unchanged:

- `require_auth` is the JWT layer.
- `settings:providers` is the existing permission layer.
- Any field not shown is unchanged.

## Catalogue: read-only, any authenticated user

Mounted at `/catalogue` with `require_auth` only. Agent editors need these to build pickers even without `settings:providers`. No secret is ever returned: connection state is a status and a source label only.

### `GET /catalogue`

```json
{ "source": "bundled" | "refreshed",
  "fetched_at": "2026-10-15T09:12:00Z",
  "refresh": { "enabled": true, "interval": "24h", "last_attempt": "…", "last_error": null | "…" },
  "provider_count": 230, "model_count": 8870 }
```

### `GET /catalogue/providers?q=&kind=chat|media|local|custom&connected=true`

Lists every selectable provider: catalogue providers with a supported dialect, built-ins, and custom providers. Unsupported catalogue providers are left out (FR-008).

```json
[{ "id": "anthropic", "name": "Anthropic", "origin": "catalogue" | "builtin" | "custom",
   "kind": "cloud" | "local" | "media",
   "dialect": "anthropic",
   "logo": "/api/v1/catalogue/logos/anthropic.svg" | null,
   "model_count": 9,
   "connection": { "status": "connected" | "missing" | "unreachable", "source": "saved" | "env:ANTHROPIC_API_KEY" | null },
   "env": ["ANTHROPIC_API_KEY"],
   "fields": ["api_key"] }]
```

- `fields` lists what the connect form asks for: any of `api_key`, `endpoint`, `base_url`, `access_token`, `account_id`.
- The sort order is: connected first, then custom, then the providers vizier supported before this feature (as "popular"), then A–Z.

### `GET /catalogue/providers/{id}/models?all=false`

Returns the provider's models.

- **`all=false` (default)**: tool-capable, non-deprecated models only (FR-031), sorted by `release_date` descending. The first entry has `"default": true`.
- **Local providers**: the list is merged with the server's live list. Live-only models carry `"origin": "server"`.
- **Custom providers**: the operator's model list, with catalogue fill-in marked per field.

```json
[{ "id": "claude-sonnet-4-5", "name": "Claude Sonnet 4.5", "default": true,
   "logo": "/api/v1/catalogue/logos/anthropic.svg",
   "limit": { "context": 200000, "input": null, "output": 64000 },
   "caps": { "tool_call": true, "reasoning": true, "image_input": true, "attachment": true },
   "cost": { "input": 3.0, "output": 15.0, "cache_read": 0.3, "cache_write": 3.75 },
   "status": "active" | "beta" | "alpha" | "deprecated",
   "release_date": "2025-09-29", "knowledge": "2025-02",
   "filled_from_catalogue": ["limit.context", "caps.tool_call"] }]
```

`filled_from_catalogue` appears on custom providers only.

### `GET /catalogue/providers/{id}/models/{model}`

Returns one model, in the same shape as above. A model id that isn't listed returns `404` with `message: "not in catalogue"`, and the form still accepts it (FR-011).

### `GET /catalogue/logos/{id}.svg`

This route needs no auth, because `<img>` tags send no token. Logos are public data.

- **Headers**: `Content-Type: image/svg+xml`, `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'`, `X-Content-Type-Options: nosniff`, `Cache-Control: public, max-age=86400`.
- **Unknown id**: `404`.

### `POST /catalogue/refresh` (requires `settings:providers`)

Triggers a refresh now.

- **On success**: returns the new `GET /catalogue` body.
- **On failure**: `502`, with `message` holding the reason. The previous catalogue stays in use (FR-003).
- **Refresh disabled in config**: `409`.

## Providers: credentials, requires `settings:providers`

`{id}` is a `ProviderId` string. Legacy names (`gemini`, `opencode_zen`, …) are accepted and normalised, so old clients keep working (FR-015).

### `GET /providers`

Returns connected providers only (saved or from env):

```json
[{ "id": "openai", "origin": "catalogue", "source": "saved" | "env:OPENAI_API_KEY",
   "has_api_key": true, "endpoint": null, "base_url": null,
   "migrated_from": "gemini" | null,
   "used_by": { "chat": 3, "stt": 0, "tts": 1, "image_gen": 0, "embedding": 2 },
   "reachable": null | true | false }]
```

- **`used_by`** counts agents per feature (FR-010b). Dream and read-image count as `chat`.
- **`reachable`** is set only for local and custom providers. The check runs in the background with a timeout of at most 2 seconds and is cached for 30 seconds, so this endpoint never waits on it (FR-010c).

### `PUT /providers/{id}`

Body: `ProviderCredentials` (data-model §4).

- `400` if a field the provider doesn't use is sent, or if `base_url` is sent for a cloud provider (FR-027).
- An omitted secret keeps the stored value; an empty string clears it.

### `DELETE /providers/{id}?confirm=true`

- Without `confirm`, while `used_by` is non-zero: `409`, with `data: { "agents": [{"id","name","features":[…]}] }`, which the WebUI uses for its warning (FR-010b).
- With `confirm=true`: deletes the saved credentials. Env-var credentials can't be deleted, and the response says so.

### `dummyplug` (unchanged; `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`)

- `PUT /providers/dummyplug` returns `400 "dummyplug requires no configuration"`.
- `GET /providers/dummyplug` returns `404`.
- It is never listed in `GET /providers`.
- It is listed in `GET /catalogue/providers` as `origin: "builtin"`, with name `dummyplug (testing)` and connection status `connected`.

### `POST /providers/{id}/test`

Body: an optional `ProviderCredentials`, to test before saving. Makes one cheap call: the model listing, or a 1-token completion on the Anthropic dialect.

```json
{ "ok": true, "model_count": 4 }  |  { "ok": false, "status": 401, "error": "Unauthorized" }
```

## Custom providers, requires `settings:providers`

### `GET /providers/custom`

Returns each custom provider as data-model §5, minus secrets:

```json
[{ "id": "home-vllm", "name": "Home vLLM", "protocol": "openai_compatible",
   "base_url": "http://10.0.0.5:8000/v1", "has_api_key": true,
   "headers": ["x-opencode-session"],
   "models": [{ "id": "qwen3.5-32b-instruct", "context": 131072, "output": 8192,
                "tool_call": true, "image_input": false, "reasoning": true, "edited": false }],
   "models_fetched_at": "…", "used_by": { "chat": 2 }, "reachable": true }]
```

`headers` lists header names only; the values are never returned.

### `POST /providers/custom`

Body: `{id?, name, protocol, base_url, api_key?, headers?: {name: value}, models?}`.

- `id` defaults to a slug of `name`.
- `409` if the id collides with a catalogue, built-in or custom id. The message names the clash.
- `400` for an invalid id or URL.

### `PUT /providers/custom/{id}`

Same body as `POST`.

- `id` in the body must match the path or be absent (the id is immutable).
- An omitted `api_key` or header value keeps the stored value. To remove a header, set its value to `null`.
- A `models` row that differs from the stored row is saved with `edited: true`.

### `DELETE /providers/custom/{id}?confirm=true`

Behaves like `DELETE /providers/{id}`.

### `POST /providers/custom/{id}/fetch-models`

Calls the server's model listing (`GET {base_url}/models` for OpenAI-compatible, `GET {base_url}/v1/models` for Anthropic) and merges by id (data-model §5). Returns the updated provider.

- An unreachable server returns `502` with the error. Nothing is changed.

### `POST /providers/custom/test`

Body: an unsaved custom-provider body. Same response as `POST /providers/{id}/test`.

## Agents: changed fields only

- `provider`, `dream_provider` and `tools.read_image_settings.provider` are `ProviderId` strings. Legacy names are accepted.
- `context_window` and `max_tokens` are optional overrides.
- `embedding.api_key` and `embedding.base_url` are ignored if sent.
- `POST` and `PUT /agents` responses gain `warnings: [{"code": "no_tool_calling" | "no_image_input" | "unknown_model", "message": "…"}]` (FR-013, FR-035). Warnings never block a save.
- `GET /agents/health` already exposes `last_error`. An agent on a removed provider shows `provider "galadriel" is not available; pick another provider`.

## Usage: changed fields only

`GET /agents/{id}/usage` adds `estimated_cost_usd: number | null` to `summary`, to each entry of `by_day`, and to each entry of `by_channel_type` (FR-021). It is `null`, not `0`, when no price is known.
