# Contract: HTTP routes

All routes are under `/api/v1` and behind `require_auth`. They are mounted **only** when the runtime runs with `RuntimeOptions.apps = true` (research D1), and they are served the same way through the network router (desktop web access) and the embedded dispatcher (the desktop window).

Errors use the existing envelope (`vizier-http/src/error.rs`). An app the caller does not own is **`404 not_found`**, the same as an app that does not exist (FR-002).

## Capabilities

`GET /utils/capabilities` → `200 {"apps": bool}`. This route is always mounted. The WebUI shows app surfaces only when `apps` is `true`.

## Apps

| Method & path | Body | Response | Notes |
|---|---|---|---|
| `GET /apps` | | `[AppSummary]` | The caller's apps |
| `POST /apps` | `{"driver": agent_id?}` | `201 AppDetail` | Creates a **draft**. `driver` defaults to the caller's most recently used agent the caller may chat with |
| `POST /apps/from-page` | `{"agent_id", "channel_id", "topic_id", "page_id", "version"}` | `201 AppDetail` | Add to rail: copies that ui_page version as v1, with the page's agent as driver. The caller must own the conversation |
| `GET /apps/{id}` | | `AppDetail` | |
| `PATCH /apps/{id}` | `{"name"?, "icon"?, "description"?}` | `AppDetail` | Rename etc. by the owner |
| `DELETE /apps/{id}` | `?if_draft_unused=1` optional | `204` | With the flag: refused `409 draft_in_use` once any of its conversations has a request (research D14) |
| `GET /apps/{id}/versions` | `?before&limit` | `[VersionSummary]` | |
| `POST /apps/{id}/current` | `{"version": n}` | `AppDetail` | Rollback / forward. Data untouched (FR-017) |
| `GET /apps/{id}/frame-ticket` | `?version` | `{"ticket", "expires_at_ms"}` | For `/ui-frames/{ticket}` |
| `GET /apps/{id}/drivers` | | `[DriverOption]` | Every agent the caller may chat with, each with its `Report` (research D8) |
| `PUT /apps/{id}/driver` | `{"agent_id": string\|null}` | `AppDetail` | Opens a new conversation with the new driver. Approvals: non-sensitive ones are carried over (data-model) |
| `GET /apps/{id}/approvals` / `PUT /apps/{id}/approvals` | `{"function", "granted": bool}` | `[Approval]` | Owner consent per function for the current driver |
| `GET /apps/{id}/schedules` | | `[ScheduleSummary]` | With last runs and open episodes |

```jsonc
// AppSummary (rail tiles, lists)
{"id":"ap_…","name":"Habits","icon":"square-check","draft":false,
 "driver":{"agent_id":"archivist","name":"Archivist"} /* or null */,
 "badge":3,"status":"ready" /* ready | no_driver | missing */}

// AppDetail = AppSummary + 
{"description":"…","current_version":4,"latest_version":4,
 "conversations":{"channel_id":"app__ap_…","last_topic":"…"},  // list/open through the existing topic routes with this channel id
 "abilities":["state","send","files","worker","db","fn","chat","tools"],
 "manifest":{ /* contracts/manifest.md, current version */ },
 "report":{"ready":false,"gaps":[{"requirement":"secret:CAL_TOKEN","reason":"not set","fix":{"kind":"secrets","agent_id":"quill"}}]}}

// DriverOption
{"agent_id":"quill","name":"Quill","current":false,"report":{ /* as above */ }}
```

## Page abilities (called by the host for the frame)

| Method & path | Body | Response | Rules |
|---|---|---|---|
| `POST /apps/{id}/db/query` | `{"sql", "params": [..], "mode": "read"\|"write"}` | `{"columns", "rows", "changes"?}` | DDL refused. Max 2 s and 5,000 rows. A write publishes `app_data_changed` |
| `GET /apps/{id}/db/tables` | | `[{"name", "rows"}]` | Owner view (FR-018b) |
| `GET /apps/{id}/db/download` | | `application/vnd.sqlite3` | A consistent snapshot (`VACUUM INTO` a temp file) |
| `POST /apps/{id}/functions/{name}/call` | `{"input": {..}}` | `{"status":"ok","output"}` \| `{"status": "not_approved"\|"unavailable"\|"no_driver"\|"rate_limited"\|"error", "reason"}` | Runs in the driver's host through `AgentsClient::app_function` |
| `POST /apps/{id}/chat` | `{"text", "attachments": [file ids], "gesture_at_ms"}` | `202 {"request_id"}` | Gesture window and rate limits; refused `no_driver`. Becomes a `Chat` with `app_origin` (research D12) |
| `GET /apps/{id}/files` | | `[AppFile]` | App files + the uploads of conversation `?topic` (FR-021) |
| `POST /apps/{id}/files` | multipart `file`, `?to=app\|conversation&topic` | `AppFile` | `conversation` adds a session file to that conversation |
| `POST /apps/{id}/files/{fid}/share` | | `{"session_file_id"}` | Copies an app file into conversation `?topic` |
| `GET /apps/{id}/files/{fid}` | | bytes | Also through `vizier-asset://` on desktop |

## Rail order

| Method & path | Body | Response |
|---|---|---|
| `GET /me/rail-order` | | `{"items":[{"kind":"agent"\|"app","id"}]}` |
| `PUT /me/rail-order` | same | `204` |

`items` only orders. It never grants: unknown or invisible ids are ignored on read (data-model, the `rail_order` read rule).

## Socket frames (topic socket of an `app__…` channel)

These are server-to-client and added to the existing set:

- `app_data_changed {"app_id", "tables":[..]}` relays `AppEvent::DataChanged`.
- `app_version_saved {"app_id", "version"}` makes the open app reload its frame.
- `app_driver_changed {"app_id", "driver"}`
- `app_badge {"app_id", "badge"}`

The inbox stream also carries `app_badge` and `app_driver_changed` for rail tiles.

## Notifications

No new route. A reply in an app conversation is an ordinary `chat_reply` row: `href` `/apps/{app_id}/{topic}`, title `<app name> · <conversation title>` (filled on read), audience the owner. `GET /notifications/chats` lists app conversations like any other, and `POST /notifications/chats/read` with that href marks them read when opened.
