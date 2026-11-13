# Contract: HTTP routes and session frames

Replaces `specs/012-background-subagent-results/contracts/http-api.md`. All routes are under `/api/v1`, behind `require_auth`, and gated exactly as the job routes were: the person must be able to view the agent and own the conversation (`service::detached`, in core, per "new authorization goes in core").

## Routes (`vizier-http/src/api/v1/agents/detached.rs`)

Read-only in this feature. **Cancelling from the WebUI is deferred** (see "Deferred" below): only the agent cancels, with `cancel_detached_script`.

| Method and path | Purpose | Responses |
|---|---|---|
| `GET /agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/detached-scripts` | The topic's detached scripts (running first, then the last 50 final ones), as `Vec<DetachedScriptSnapshot>` | 200; 404 for a conversation the person cannot see |
| `GET …/detached-scripts/{script_id}` | One script with its code, tool calls and, once final, its report | 200 `DetachedScriptDetail`; 404 |

`DetachedScriptSnapshot` (the list and the frame: `intent`, `state`, times, `tool_call_count`) and `DetachedScriptDetail` (the snapshot plus `code`, `tool_calls` and the final `report`) are in `data-model.md`. The panel fetches the detail when an entry is first expanded and refetches it when a frame says the script changed. Removed: every `…/jobs…` route, including `pieces/{ordinal}/history` and the job cancel route.

## Session frame

`SessionFrame::DetachedScript(DetachedScriptSnapshot)` replaces `SessionFrame::Job`. It is published on every state change of a script (launch, final, cancel) over the existing session-event broadcast, after the database commit, and reaches every socket subscribed to the session. The topic socket sends it as:

```json
{ "type": "detached_script", "script": { "id": "d_k3x9a1b2", "intent": "check both vendor pages", "state": "running", … } }
```

and the WebUI's `lib/frames.ts` reducer keeps a map of scripts by id (a final script stays in the map for the panel's Done tab). A reopened page reads the list route first, then applies frames.

## Notification

A final, non-cancelled script creates one notification (`category: background_work`, tone by state, `href` to the conversation), through the one `Notifications` creation path; wording in `notifications/text.rs`. Cancelled scripts and Discord/Telegram conversations create none. Existing `background_job` notification rows are deleted by the migration that drops the job tables, so no client sees the old category.

## OpenAPI

New and changed schemas are registered with `utoipa`; `webui/scripts/openapi.mjs` regenerates `webui/openapi.json` and `app/api/schema.d.ts`; `tests/conformance/` gains one vector file `groups/detached.rs` (both routes are reads, so every vector is state-free).

## Deferred

Cancelling a script from the WebUI (a `POST …/cancel` route and a trash icon in the panel) is left for a follow-up. In a Fleet the HTTP gateway is not the process that runs the script, so a person's cancel has to travel to the owner through `AgentsClient` and the bus; that path is designed with the follow-up, not here. The agent's own `cancel_detached_script` runs in the owning process and needs none of it.
