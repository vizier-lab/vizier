# Contract: App IPC (window ⇄ Rust shell)

Everything the WebUI's `tauri.ts` transport may call. Commands are Tauri 2 commands exposed to
the main window only (capability `main`); nothing here is reachable from a remote origin.

## 1. `api` — one request through the route table

Dispatches in memory through `vizier_http::embedded::Dispatcher` (research R1, R4).

**Invoke**

| Part | Value |
|---|---|
| headers | `x-method` (`GET`…`DELETE`), `x-path` (`/agents/…?query`, relative to `/api/v1`), `content-type` (if a body) |
| body | raw bytes (`InvokeBody::Raw`): JSON text, or a `multipart/form-data` body built with `new Response(formData)` |

**Returns** JSON `{ "status": <u16>, "body": <the server's envelope or null> }`.
`body` is exactly what the network route would have returned. A non-JSON response (only the
asset routes produce one) is refused with `status: 406` and the caller uses `vizier-asset://`.

**Errors**: none at the IPC level for HTTP-level failures (they are `status` + envelope). An
IPC error means the shell itself failed (engine not ready: `{"code":"engine_not_ready"}`).

## 2. `vizier-asset://localhost/api/v1/<path>` — binary GETs

Custom URI scheme. GET only. Forwards `Range`; returns status, `content-type`,
`content-length`, `content-range`, `accept-ranges`, `content-disposition` from the dispatcher.
Adds `access-control-allow-origin` for the app's own origin. Any other method → 405.

## 3. Topic streams (replaces the chat WebSocket)

| Command | Args | Returns |
|---|---|---|
| `topic_open` | `{agent_id, channel_id, topic_id, frames: Channel<Value>}` | `{id}` or `{status: 403/404}` like the WS upgrade |
| `topic_send` | `{id, frame: Value}` | `()`; the frame set accepted by the WebSocket today |
| `topic_close` | `{id}` | `()` |

Frames on the channel are the JSON values the WebSocket sends today, in order. Closing the
window closes every open topic.

## 4. Desktop shell

| Command | Args | Returns / effect |
|---|---|---|
| `desktop_state` | — | `{state: "starting" \| "locked" \| "needs_owner" \| "choose_owner" \| "ready" \| "degraded" \| "failed", detail}` (data-model state machine) |
| `desktop_setup_owner` | `{username, password?}` | creates a user with the built-in system role, binds it → `ready` |
| `desktop_choose_owner` | `{user_id}` | binds → `ready` |
| `desktop_open_workspace` | `{path?}` (none → folder picker) | stops the engine, switches workspace, restarts |
| `desktop_settings_get` / `desktop_settings_set` | `{close_to_tray, web_access{enabled, allow_network, port}, updates{check_on_start}}` | `set` refuses `web_access.enabled = true` with `{code: "password_required"}` when the bound user has neither a password nor an API key |
| `runtime_restart` | — | shutdown + start on the same workspace |
| `save_file` | `{path, filename}` | native save dialog, streams `vizier-asset` GET to disk |
| `open_file` | `{path, filename}` | saves to a temp file, opens with the default app |
| `notify` | `{title, body, href}` | OS notification; selecting it focuses the window and emits `desktop://navigate` |
| `update_check` / `update_install` | — | updater plugin; install relaunches |

## 5. Events (shell → window)

| Event | Payload |
|---|---|
| `desktop://state` | same as `desktop_state` |
| `desktop://navigate` | `{href}` (from a notification, a `vizier://` link or the tray) |
| `desktop://web-access` | `{running, url?, error?}` |

## 6. Deep links

`vizier://<route>` (e.g. `vizier://agents/dp-a/chat`) → focus window, emit
`desktop://navigate {href: "/<route>"}`. Unknown routes land on the WebUI's own not-found
redirect.

## 7. Guarantees

- No command accepts a user id or credential to act as; every `api`/`topic_*`/asset call runs
  as the bound desktop principal, re-resolved per call.
- Navigation away from the app origin is cancelled and opened in the default browser.
- Before `ready`, `api`, `topic_*` and the asset scheme answer `engine_not_ready`.
