# Contract: HTTP routes and session frames

Replaces `specs/012-background-subagent-results/contracts/http-api.md`. All routes are under `/api/v1`, behind `require_auth`, and gated exactly as the job routes were: the person must be able to view the agent and own the conversation (`service::detached`, in core, per "new authorization goes in core").

## Routes (`vizier-http/src/api/v1/agents/detached.rs`)

| Method and path | Purpose | Responses |
|---|---|---|
| `GET /agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/detached-calls` | The topic's detached calls (running first, then the last 50 final ones), as `Vec<DetachedCallSnapshot>` | 200; 404 for a conversation the person cannot see |
| `GET …/detached-calls/{call_id}` | One call with its full outcome | 200 `DetachedCallDetail`; 404 |
| `POST …/detached-calls/{call_id}/cancel` | Cancel a running call and the calls launched from it. Body `{ "reason"?: string }` (≤ 200 characters) | 200 with the cancelled snapshot; 409 with the snapshot if it was already final; 404 |

`DetachedCallSnapshot` (the list and the frame: `intent`, `state`, times, `tool_call_count`) and `DetachedCallDetail` (the snapshot plus `code`, `tool_calls` and the final `report`) are in `data-model.md`. The panel fetches the detail when an entry is first expanded and refetches it when a frame says the script changed. Removed: every `…/jobs…` route, including `pieces/{ordinal}/history`.

## Session frame

`SessionFrame::DetachedCall(DetachedCallSnapshot)` replaces `SessionFrame::Job`. It is published on every state change of a call (launch, final, cancel) over the existing session-event broadcast, after the database commit, and reaches every socket subscribed to the session. The topic socket sends it as:

```json
{ "type": "detached_call", "call": { "id": "d_k3x9a1b2", "intent": "check both vendor pages", "state": "running", … } }
```

and the WebUI's `lib/frames.ts` reducer keeps a map of calls by id (a final call stays in the map for the tray's Done tab). A reopened page reads the list route first, then applies frames.

## Notification

A final, non-cancelled call creates one notification (`category: background_work`, tone by state, `href` to the conversation), through the one `Notifications` creation path; wording in `notifications/text.rs`. Cancelled calls and Discord/Telegram conversations create none.

## OpenAPI

New and changed schemas are registered with `utoipa`; `webui/scripts/openapi.mjs` regenerates `webui/openapi.json` and `app/api/schema.d.ts`; `tests/conformance/` gains one vector file `groups/detached.rs` (vectors must not change state: the harness sends each twice, so cancel is exercised only against an already-final call).
