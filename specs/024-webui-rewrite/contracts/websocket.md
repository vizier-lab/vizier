# Contract: WebSocket frames the new WebUI handles

Socket: `GET /api/v1/agents/{id}/channel/{channel}/topic/{topic}/chat` (upgrade). The protocol is unchanged except for one new frame. The public description stays `docs/src/api-integration/client-tools.md` plus the API docs. This file is the list the WebUI's `app/api/frames.ts` types by hand, and which `lib/frames.ts` reduces.

## Server → client

| Frame | Since | WebUI use |
|-------|-------|-----------|
| `thinking_start` | — | Turn state → Thinking; Send becomes Stop |
| `thinking` / `tool_choice` / `tool_response` | 010 | Live labels only ("Archivist is running execute_python…"); the trail itself comes from `history_entry` |
| `history_entry` | 023 | Merge by uid into the topic's entries: trail steps, client tool calls, **recall** entries (new content type, FR-018e) |
| `response` | — | Reply lands: trail folds, footer stats, `stats.current_context_size`/`context_window` feed the context ring, `resumed` marks a resumed reply |
| `error` (`kind`: `rate_limited`, `provider_unavailable`, `agent_unavailable`, `tool_timeout`, `prompt_timeout`, `completion`, …) | 015/#52 | The matching Notice in place of the reply, with its action (FR-008) |
| `job` | 012 | Background jobs panel and JobsPill |
| `reactions` | 013 | Reply reactions (also the ack of the person's own) |
| `client_tool_call`, `client_call_outcome`, `client_call_deferred`, `client_call_state`, `client_tools_resolved`, `client_error` | 019 | Client tools, unchanged behaviour |
| **`session_file`** | **024** | **New.** `{ type: "session_file", file: SessionFileRecord }`, sent when a file is added to the topic. The Session files panel prepends it, and FilePeek can link to it |

## Client → server

Unchanged: chat requests (`text`, attachments, `client_tools`/`client_categories` declarations), `client_event`, client tool results and state saves, and `/abort` (as chat text). The WebUI sends the client tool declarations from `lib/clientTools.ts` on every request, as legacy does.

## Rules the WebUI keeps

- One socket per open topic. On close, the state goes to `reconnecting`, with backoff (1s doubling to 30s), and then `offline` after 3 failures, which shows the Banner. On reopen, it re-reads history since the last uid it has, then follows frames (no duplicate steps: merge by uid).
- Turns are published on the session-event broadcast, so a second tab, or a page reopened mid-turn, gets the same frames. The WebUI never assumes the socket that sent a message is the one that receives its reply.
