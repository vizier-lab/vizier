# Contract: page verification and page frames

## Bus

| Subject | Kind | Payload |
|---|---|---|
| `ui.{session_key}.verify` | request/reply | request `{request_id, page_id, version, observe_ms}` → reply `{request_id, problems: [Problem], observed_ms}` or `{request_id, closed: true}` |

`session_key` is the session's slug (`Subject::ui_verify`), as in `session.{id}.events`.
- Each chat socket on a `HTTP` or `Project` session subscribes for as long as it is open.
- The tool's request has a deadline of 10 s.
- `NoResponders` produces `unverified(no_tab)`, `Timeout` produces `unverified(timeout)`, and a `closed` reply produces `unverified(closed)`.
- The first reply wins.

## Socket frames (topic socket)

**Outbound (server → tab):**

| type | payload | notes |
|---|---|---|
| `history_entry` | the `UiPage` entry | existing frame (`record_step`) |
| `ui_verify` | `{request_id, page_id, version, observe_ms}` | sent only to the socket the bus request reached |
| `ui_page_verified` | `{page_id, version, verification}` | `SessionFrame::UiPageVerified`, to every socket on the session |
| `ui_page_state` | `{page_id, state, state_seq}` | `SessionFrame::UiPageState`, to every socket on the session |

**Inbound (tab → server):**

| type | payload | notes |
|---|---|---|
| `ui_verify_report` | `{request_id, problems, observed_ms}` | ignored if no such request is waiting on this socket |
| `ui_page_state` | `{page_id, state, expected_seq?}` | same checks and rate limit as `client_call_state`; refused for a page outside the session |
| `client_event` | existing frame, plus `page: {page_id, version}` | the person's turn from a page |

## Problem

```json
{ "kind": "error|rejection|console|blocked|kit|unresponsive",
  "message": "…", "source": "page|kit", "line": 0, "column": 0, "detail": "…" }
```

- At most 20 problems per report, plus `"more": N`.
- `message` and `detail` are cut to 500 characters each.

## Tab behaviour on `ui_verify`

1. Fetch the version's HTML (`GET …/pages/{page_id}/versions/{version}`).
2. Mount it in the off-screen verifier (1024×768, hidden) with the page's current state.
3. Collect problems for `observe_ms`. No heartbeat for 1 s produces `unresponsive`.
4. Send `ui_verify_report`, then unmount.

If the tab is closing, it sends nothing. The bus deadline covers that case.
