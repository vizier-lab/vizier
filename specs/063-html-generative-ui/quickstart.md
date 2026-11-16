# Quickstart: verify 063 end to end

## Setup

- Run it offline, on a `dummyplug` agent (no keys).
- Start the server with `just run`, create an agent on the `dummyplug` provider in the WebUI, and open a WebUI chat with it.
- Each `{…}` message is the dummyplug §3 JSON request, and its reply is the §4 tool-result echo.
- `S(code)` means `{"tool":"execute_python","arguments":{"intent":"test","code":"<code>"}}`, with the code JSON-escaped.
- `H` is a small page, `<h1>Hi</h1><button id=b>Go</button>`.

**Automated checks:**
- Steps 1, 2, 4 and 6 should run as `cargo test -p vizier-core --test dummyplug_e2e ui_pages`.
- Steps 3 and 5 should run as `node scripts/ui-pages-e2e.mjs`. It opens a topic socket as a fake tab that answers `ui_verify` from a script.
- Step 7 is a manual browser check, done with the preview tools.

## 0. Availability (Story 6)

| Where | Send | Expect |
|---|---|---|
| WebUI chat, **no tab open** (send through `scripts/ui-pages-e2e.mjs --no-tab`) | `context` | `SANDBOX.md` lists category `ui` |
| WebUI chat | `{"tool":"describe_tool_function","arguments":{"name":"ui_render"}}` | doc with a non-empty `guide` that starts with the page model |
| scheduled task whose message is `context` (create the task, wait for its run, read `…/tasks/{slug}/runs/{id}/history`) | — | no `ui` category |
| Discord or Telegram (skip if not configured) | `context` | no `ui` category |
| WebUI chat | `S(ui_components())` | `RuntimeError` with the retired-tool guidance naming `ui_render` |

## 1. Create, with no tab (Story 1, FR-019)

Close every tab on the conversation and drive it through the script's socket with `--no-tab`.

| Send | Expect |
|---|---|
| `S(ui_render(title="Hello", html=H))` | `{"status":"rendered","page":"pg_…","version":1,"verified":false,"reason":"no_tab"}`; the turn ends (`ended_by_tool: "ui_render"`) |
| `GET …/topic/{t}/pages/{page}` | `latest_version: 1`, one version, `origin: "full"`, `verification.state: "unverified"` |
| `GET …/history` | one `UiPage` entry `{page_id, version: 1, title: "Hello", verification}` |

## 2. Edits and refusals (Story 2, FR-007 to FR-011)

| Send | Expect |
|---|---|
| `S(ui_render(page=P, base_version=1, edits=[{"find":"Hi","replace":"Hello there"}]))` | `version: 2`; blob `v2.html` contains `Hello there`; `origin: "edits"`, `base_version: 1` |
| same again (base 1) | `RuntimeError`, `code: "stale_base"`, `latest: 2`; no version 3 |
| `S(ui_render(page=P, base_version=2, edits=[{"find":"nope","replace":"x"}]))` | `code: "edit_not_found"`, `edit: 0` |
| `S(ui_render(page=P, base_version=2, edits=[{"find":"<","replace":"&lt;"}]))` | `code: "edit_ambiguous"`, `count` > 1 |
| `S(ui_render(page=P, base_version=2, edits=[{"find":"Hello there","replace":"A"},{"find":"missing","replace":"B"}]))` | `edit_not_found`, `edit: 1`; still `latest_version: 2` (all or nothing) |
| `S(ui_render(page=P, base_version=2, html="x"*1048577))` | `code: "too_large"`, `size`, `limit: 1048576` |
| `S(ui_render(title="x"))` | `code: "invalid_input"` |
| In a **second** WebUI conversation: `S(ui_render(page=P, base_version=2, edits=[…]))` | `code: "other_conversation"` |

## 3. Verification (Story 3, FR-017 to FR-020); fake tab via `scripts/ui-pages-e2e.mjs`

| Fake tab answers | Send | Expect |
|---|---|---|
| `ui_verify_report` with no problems | `S(ui_render(title="Ok", html=H))` | `verified: true`; `ui_page_verified` frame `state: "clean"`; the turn ends |
| report with `[{kind:"error", message:"boom", line:3}]` | `S(ui_render(title="Bad", html=H))` | `RuntimeError` `code: "problems"`, `page`, `version: 1`, the problem; version **saved**; the turn **continues** |
| (same turn) clean report | `S(ui_render(page=<that page>, base_version=1, edits=[…]))` | `verified: true`; the turn ends; history has two `UiPage` entries in that turn |
| silence | `S(ui_render(title="Slow", html=H))` | after about 10 s, `verified: false`, `reason: "timeout"` |
| two fake tabs, the first clean and the second with problems | `S(ui_render(title="Two", html=H))` | `verified: true` (first reply wins) |

## 4. State across versions (Story 5, FR-027)

| Step | Expect |
|---|---|
| fake tab sends `ui_page_state {page_id: P, state: {"n": 1}}` | other sockets receive `ui_page_state` with `state_seq: 1` |
| `S(ui_render(page=P, base_version=2, edits=[…]))` then `GET …/pages/P` | `state` is still `{"n": 1}` (shared across versions) |
| send 5 states within 1 s | at most 2 stored; the rest answered `rate_limited` |
| `ui_page_state` for a page of another conversation | refused |

## 5. Talk back (Story 5, FR-028)

| Step | Expect |
|---|---|
| fake tab sends `client_event {text:"e2e4", data:{"from":"e2","to":"e4"}, page:{page_id:P, version:3}}` | a new turn starts; dummyplug `context` in it shows frontmatter `via: client_event` and `page: P v3` |

The host-side gesture and rate rules are checked in step 7, because they are enforced in the browser.

## 6. Deletion and recovery (FR-012)

| Step | Expect |
|---|---|
| delete the conversation | `GET …/pages/P` is `404`; no rows remain in `ui_page` or `ui_page_version`; blob prefix `ui-pages/{agent}/P/` is empty |
| create a page, then delete the agent | same for every page of that agent |
| leave a version `pending` (kill the process mid-verification), restart | the row reads `unverified(closed)`; no duplicate version |

## 7. In a real browser (preview tools)

1. **Game.** Live provider, marked as such: ask for a playable chess board. The board draws, moves work, and full screen and back work.
2. **Isolation suite.** `webui/app/test/fixtures/hostile-pages/`, one page each for: `localStorage`, `document.cookie`, `parent.document`, the token in `top.location`, `fetch`, `WebSocket`, `<script src="https://…">`, and `window.open`. Every probe reports blocked, and the WebUI is unaffected (SC-008).
3. **Media.** `<img src="https://…">` loads with no prompt. The same image over `http://` is blocked and appears in verification as `kind: "blocked"`.
4. **Theme.** A page with no styles matches the chat in light and dark. Switching the theme recolours it without a reload, and its state is kept.
5. **Kit.** `<vz-stat label="Revenue" value="12k">` and `<vz-chart>` draw. `<vz-stat value>` with no label shows an inline problem and a `kit` problem.
6. **Error indicator.** A button whose handler throws lights the frame's indicator after a click, and the popover lists the error.
7. **Send rules.** `vizier.send` on load rejects with `no_interaction`. It works after a click, and a second send within 1 s rejects with `rate_limited`.
8. **Hang.** `while(true){}` on load:
   - Chromium: the chat stays responsive and verification reports `unresponsive`.
   - WebKit: note the behaviour (research R7 risk).
9. **Phone.** At 375 px wide the page fits and full screen works.
10. **Old pages.** A workspace with saved `render_interactive_ui` and json-render `ui_render` pages draws both, with their state (SC-006).
11. **Download.** The downloaded file opens offline and keeps the kit look.
