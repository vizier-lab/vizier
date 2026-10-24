# Contract: Media Catalogue API

**Feature**: `028-openrouter-media-provider` | **Plan**: [../plan.md](../plan.md)

Two read/refresh endpoints under the existing `/api/v1/catalogue` router. They return OpenRouter's OpenRouter-only media listings; the chat/model catalogue endpoints are unchanged.

## Authentication and permissions

| Endpoint | Auth | Permission |
|---|---|---|
| `GET /api/v1/catalogue/media-models` | `require_auth` | any signed-in user |
| `POST /api/v1/catalogue/media-models/{capability}/refresh` | `require_auth` + `require_permission` | `settings:providers` |

The list itself does **not** require an OpenRouter credential. This is deliberate: a person must be able to configure an agent before connecting the provider, and a fresh/offline install uses the seed. The refresh is a provider-settings action, matching `POST /catalogue/refresh`.

No secret is ever returned in either response.

## `GET /api/v1/catalogue/media-models`

### Query

| Name | Type | Required | Values |
|---|---|---|---|
| `capability` | string | yes | `speech`, `transcription`, `image` |

Unknown or missing capability → `400` with `"capability must be one of: speech, transcription, image"`.

### Behaviour

1. Parse the capability.
2. Call `maybe_refresh(deps, capability)`:
   - if `catalogue.refresh` is false → do nothing;
   - if `now - (checked_at.unwrap_or(fetched_at)) < catalogue.ttl` → do nothing;
   - if a failure happened less than 1 h ago → do nothing;
   - if a check is already running → do nothing;
   - otherwise mark in progress and spawn one background check. The response is served immediately from memory.
3. Build the listing from the current seed/cache (never waits on the network).
4. Mark the documented default and inject it with `listed: false` if absent.
5. Return `200`.

A stale list plus `refresh.in_progress: true` is a valid response; the UI may show a quiet “refreshing” note and refresh later. The endpoint never returns 5xx for a refresh failure; the previous/seed list is served with `refresh.last_error`.

### Response `200`

```json
{
  "status": 200,
  "message": null,
  "data": {
    "capability": "speech",
    "source": "bundled",
    "fetched_at": "2026-10-24T00:00:00Z",
    "checked_at": null,
    "default_voice": "af_alloy",
    "refresh": {
      "enabled": true,
      "ttl": "86400s",
      "in_progress": false,
      "last_error": null,
      "failed_at": null
    },
    "models": [
      {
        "id": "hexgrad/kokoro-82m",
        "name": "hexgrad: Kokoro 82M",
        "default": true,
        "listed": true,
        "voices": ["af_alloy", "af_aoede", "af_bella"],
        "aspect_ratios": [],
        "resolutions": []
      }
    ]
  }
}
```

The outer envelope is the existing `APIResponse<T>` (`status`, `message`, `data`), not part of the feature type.

### Fields

| Field | Type | Notes |
|---|---|---|
| `capability` | string | Echo of the query. |
| `source` | `bundled` \| `cached` | `bundled` = the embedded seed; `cached` = a successfully refreshed/stored list. |
| `fetched_at` | RFC 3339 string | When the current bytes were fetched. |
| `checked_at` | RFC 3339 string \| null | Last successful check; null for the seed until a refresh. |
| `default_voice` | string \| null | The documented default voice for `speech` (`af_alloy`); `null` for transcription/image. |
| `refresh.enabled` | bool | `catalogue.refresh`. |
| `refresh.ttl` | string | `catalogue.ttl.to_string()` (e.g. `86400s`), same as the chat catalogue. |
| `refresh.in_progress` | bool | One check per capability. |
| `refresh.last_error` | string \| null | Last failure reason, restored from storage at startup. |
| `refresh.failed_at` | RFC 3339 string \| null | Last failure time. |
| `models[].id` | string | OpenRouter id, exactly as published. |
| `models[].name` | string | Readable name, or the id when OpenRouter omits it. |
| `models[].default` | bool | The documented default. Exactly one entry has `true` (unless the list is empty and the default is injected, which still yields one). |
| `models[].listed` | bool | `false` only for an injected default absent from the current list. |
| `models[].voices` | string[] | Speech only; empty otherwise. |
| `models[].aspect_ratios` | string[] | Image only; empty otherwise. |
| `models[].resolutions` | string[] | Image only; empty otherwise. |

### Default injection

- Present in the list → `default: true`, `listed: true`, position unchanged.
- Absent → prepend `{ id: default, name: default, default: true, listed: false, voices: [], aspect_ratios: [], resolutions: [] }`.
- The injected entry is the effective value when the agent field is empty; it is never a silent replacement with another model.
- A speech injected default has no voices (the source did not publish them); the voice field keeps its free-text behavior with the documented default voice as placeholder.

## `POST /api/v1/catalogue/media-models/{capability}/refresh`

### Path

| Name | Type | Values |
|---|---|---|
| `capability` | string | `speech`, `transcription`, `image` |

### Behaviour

1. Invalid capability → `400` (same message).
2. `catalogue.refresh == false` → `409` with `"catalogue refresh is disabled (catalogue.refresh: false)"`.
3. If a check for this capability is in progress, wait up to 30 s for it; otherwise start one and wait.
4. Success → `200` with the same shape as the GET (now `source: "cached"`, fresh timestamps, failure cleared).
5. Failure → `502` with the reason in `message`; the previous list remains in memory and storage, and the next lazy check waits 1 h.
6. If the check is still running after 30 s, return `200` with `refresh.in_progress: true` and the current list (the UI can show “still refreshing”); no error is fabricated.

### Status codes

| Status | Meaning |
|---|---|
| `200` | Refreshed, already current, or still running after the wait. |
| `400` | Missing/invalid capability. |
| `401` | Not signed in. |
| `403` | Missing `settings:providers`. |
| `409` | Refresh disabled by config. |
| `502` | The check failed; the previous/seed list is still in use. |

## Implementation notes

- Register both paths in `ApiDoc` (`src/channels/http/mod.rs`) and the response/query types in its `components` list.
- Route placement mirrors the existing catalogue:
  - `authed`: `.route("/media-models", get(media_models))`
  - `admin`: `.route("/media-models/{capability}/refresh", post(refresh_media))`, layered with `require_permission("settings:providers")` and `require_auth`.
- The GET handler calls the refresh trigger only after parsing the capability, so an invalid request cannot start a check.
- No ETag/If-None-Match handling; a refresh is a full GET (see research R7).
- The handler reads the list through `Catalogue::media_list(capability)` and the status through `catalogue::media::status(&state.deps, capability)`; it never reads SQLite directly.
- `webui/app/api/schema.d.ts` is regenerated from the OpenAPI document after implementation.

## Compatibility

- Existing `/catalogue/*` responses and permissions are unchanged.
- No endpoint accepts or returns a provider credential.
- A typed model id is not validated here; provider validation happens on a media call (FR-021/FR-024).
