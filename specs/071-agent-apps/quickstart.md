# Quickstart: verify 071 end to end

## Setup

- **Driver agents**: everything runs offline on `dummyplug` agents (no keys). Create two: **Archivist** and **Scout**.
- **Driving the steps**: each `{…}` is the dummyplug §3 JSON request, and its reply is the §4 tool-result echo. `S(code)` means `{"tool":"execute_python","arguments":{"intent":"test","code":"<code>"}}` with the code JSON-escaped.
- **The sample page** `H`: `<h1 id=t>Habits</h1><button id=b>Add</button><script>b.onclick=async()=>{await vizier.db.exec("INSERT INTO habit(name) VALUES (?)",["Read"]);}</script>`
- **Automated runs**:
  - Steps marked **(T)** run as `cargo test -p vizier-core --features subapp --test dummyplug_e2e apps` and `cargo test -p vizier-http --test apps`, both with `RuntimeOptions.apps = true` on a temporary workspace.
  - Steps marked **(UI)** run in the desktop app (`just app-dev`) or its web access in the built-in browser, through the preview tools.

## 0. Edition and startup (FR-001, SC-009; manual-verification gate)

| Where | Do | Expect |
|---|---|---|
| (T) server router (`apps: false`) | `GET /api/v1/utils/capabilities` | `{"apps": false}`; `GET /api/v1/apps` → `404`; `context` in any chat lists no `apps` category |
| (T) apps runtime on PostgreSQL | start | refused at startup with "apps need the SQLite backend" |
| (UI) `just app-dev` on a fresh workspace, then again on an existing one | open the rail | starts cleanly; `{workspace}/.runtime/vizier.db` has the `app*` and `rail_order` tables; `+` opens the New agent / New app menu (frame H) |
| (UI) the same workspace in `just run` | open the WebUI | no + menu, no app tiles, `+` goes straight to New agent (spec 064) |

## 1. Create an app (Story 1)

| Step | Send / do | Expect |
|---|---|---|
| (T) | `POST /apps {"driver":"archivist"}` | `201`, `current_version: 0`, `name: "Untitled app"`, a `conversation` (topic id) |
| (T) | `DELETE /apps/{id}?if_draft_unused=1` with no message sent | `204`; the app is gone |
| (T) | new draft, then in its conversation `context` | `## App state` shows `draft`, no tables; `SANDBOX.md` lists category `apps` |
| (T) | `S(app_edit(base_version=0, page=H, name="Habits", icon="square-check", manifest={"migrations":{"append":["CREATE TABLE habit (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"]}}))` | `{"status":"saved","version":1,"migrations_applied":[1]}`; `GET /apps/{id}` → `draft: false`, `name: "Habits"` |
| (T) | `DELETE /apps/{id}?if_draft_unused=1` now | `409 draft_in_use` (not a draft) |
| (T) | `S(app_edit(base_version=0, page=H))` on a fresh draft | `{"status":"invalid","problems":[…name…,…icon…]}` (research D14) |
| (T) | in an ordinary WebUI chat: `S(ui_render(title="T", html=H))`, then `POST /apps/from-page {…page…}` | `201`, v1 equals that page; driver is that chat's agent; deleting the chat leaves the app |
| (T) | in an ordinary chat: `S(app_query(sql="select 1"))` | `NameError` (not offered outside an app conversation) |

## 2. Use the app (Story 2)

| Step | Send / do | Expect |
|---|---|---|
| (T) | `POST /apps/{id}/db/query {"sql":"INSERT INTO habit(name) VALUES (?)","params":["Read"],"mode":"write"}` | `{"changes":1}`; the socket gets `app_data_changed {"tables":["habit"]}`; **no** `usage_event` row added |
| (T) | `… {"sql":"DROP TABLE habit","mode":"write"}` | `refused` |
| (T) | `… {"sql":"SELECT * FROM _vizier_migrations"}` | rows readable; `INSERT INTO _vizier_migrations …` → `refused` |
| (T) | `… {"sql":"ATTACH DATABASE 'x' AS y"}` | `refused` |
| (T) | `… {"sql":"WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM c) SELECT x FROM c"}` | `timeout` or `too_large` within 2 s |
| (UI) | open Habits from the rail, press **Add** in the page, restart the app, reopen | the row is still there; chat inset on the right; the handle resizes within 320/560 px; a double-click resets; the width is remembered for this app |

## 3. Change it through the chat (Story 3)

| Step | Send | Expect |
|---|---|---|
| (T) | `S(app_edit(base_version=1, edits=[{"find":"Habits</h1>","replace":"My habits</h1>"}]))` | `version: 2`, a verification result; the open tab gets `app_version_saved` |
| (T) | `S(app_edit(base_version=1, edits=[…]))` again | `stale_base`, `latest: 2` |
| (T) | `S(app_edit(base_version=2, manifest={"migrations":{"append":["ALTER TABLE habit ADD COLUMN optional_sunday INTEGER NOT NULL DEFAULT 0"]}}))` | `version: 3`, `migrations_applied: [2]` |
| (T) | `S(app_edit(base_version=3, manifest={"migrations":{"append":["CREATE TABLE habit (x)"]}}))` | `migration_failed`; still `latest_version: 3`; `_vizier_migrations` has 2 rows |
| (T) | `POST /apps/{id}/current {"version":1}` | `current_version: 1`; the `habit` rows and the column are unchanged (FR-017) |
| (T) | `S(app_query(sql="select count(*) from habit"))` in the conversation | the same count as before the rollback |
| (T) | from a **dream** of Archivist, and from a scheduled task: `context` | no `apps` category (FR-014) |
| (UI) | a `/btw` side chat and steering a busy turn inside the app chat | both work as in an agent chat (FR-011) |

## 3b. Conversations and waking (Story 3, FR-012 to FR-012b)

| Step | Do | Expect |
|---|---|---|
| (T) | send a message on a new topic of channel `app__<id>` | a second conversation; the topic list of that channel shows both; `S(app_query(sql="select count(*) from habit"))` gives the same count in both |
| (UI) | press + at the top of the chat inset, then switch back through the list (frames J, K) | the page gets a `conversation` message each time and `vizier.conversation.id` changes |
| (T) | a scheduled task of Archivist (the driver, owned by the app's owner) whose message is `S(wake_session(target="app__<owner>__<id>", topic_id=…, instruction="context"))` | `status: ok`; a turn runs in that app conversation and its `## App state` is present |
| (T) | the same wake from Scout (not the driver) | `status: not_wakeable` |
| (T) | an `ask` schedule fires twice | both runs land in the conversation `sched-<name>`, never in the person's open one |

## 4. Switch the driver (Story 4)

| Step | Do | Expect |
|---|---|---|
| (T) | Archivist saves a function `sync` whose script calls `get_secret("CAL_TOKEN")`; Scout has no such secret | `GET /apps/{id}/drivers` → Archivist `ready` if it holds the secret; Scout `missing` with gap `secret:CAL_TOKEN` and fix `{"kind":"secrets","agent_id":"scout"}` |
| (T) | the requirement matrix test (`apps::requirements` unit tests, ≥ 10 combinations) | no false ready, no false missing (SC-005) |
| (T) | `PUT /apps/{id}/driver {"agent_id":"scout"}` | a new `conversation`; Archivist's app conversations are still readable under Archivist; `sync` reports `available: false` with the reason |
| (T) | Scout's first turn: `context` | `## App state` lists version, tables, functions and the gap |
| (UI) | open the picker from the agent's name at the top of the chat | it matches frame D |

## 5. No driver (Story 5)

| Step | Do | Expect |
|---|---|---|
| (T) | delete Scout (the current driver) | `driver: null`; one `app_driver_lost` notification; app, versions, database and files intact (SC-003) |
| (T) | `POST /apps/{id}/chat {…}` | `no_driver` |
| (T) | a due schedule | an `app_schedule_run` row `skipped`, reason `no_driver`; no notification |
| (UI) | open the app | it matches frame E: the page works, the chat is disabled, Choose a driver works |

## 6. Functions, approvals, schedules (Story 6)

| Step | Do | Expect |
|---|---|---|
| (T) | function `stamp` = `app_query(sql="insert into habit(name) values (?)", params=[current_time()["iso"]], mode="write")`, `page_callable: ["stamp"]`, **no approval** | `POST /apps/{id}/functions/stamp/call` → `not_approved`, no row (SC-006) |
| (T) | `PUT /apps/{id}/approvals {"function":"stamp","granted":true}`, call again | `ok`; one row; `FakeAgentsClient` (gateway test) or the host (e2e) saw one `app_function` |
| (T) | a new version adds `stamp2` to `page_callable` | `stamp2` → `not_approved`, `stamp` still `ok` |
| (T) | switch the driver, call `stamp` | still `ok` (it only reads time and writes the app database, so not sensitive); a function calling `get_secret` → `not_approved` after the switch |
| (T) | schedule `{"name":"tick","cron":"* * * * *","run":"stamp"}`, advance the scheduler clock | one `ok` run per minute with no app or socket open (SC-008); `app_set_badge` from a function updates the tile through `app_badge` |
| (T) | make `stamp` fail three runs in a row | **one** `app_schedule_failed` notification; the next success closes the episode |

## 7. Rail (Story 7)

| Step | Do | Expect |
|---|---|---|
| (T) | `PUT /me/rail-order` with an app between two agents, then `GET` | that order; an unknown id in the body is ignored |
| (UI) | drag Budget between Habits and Scout, reload | order kept; another signed-in person sees their own order; the keyboard can move a tile |
| (UI) | the first start after upgrading, with an old `vizier.rail.order` in localStorage | it is uploaded once and then removed (#82 shim) |

## 8. Files and messages from the page (Story 8)

| Step | Do | Expect |
|---|---|---|
| (T) | `POST /apps/{id}/files?to=conversation&topic=…` (a CSV), then in the conversation `S(app_files(action="keep", id=…))` | listed as an app file; it survives deleting the conversation and switching the driver |
| (T) | `POST /apps/{id}/chat {"text":"look","attachments":[fid],"gesture_at_ms":now}` | `202`; history shows a `Chat` request with `app_origin`; frontmatter has `via: app`; the dummyplug driver replies `context` |
| (T) | the same with `gesture_at_ms` older than 10 s | `no_gesture` |
| (T) | 25 sends in a minute | `rate_limited` from the 21st |
| (UI) | in a page with `<input type=file>` + `vizier.files.upload` + a Send button: pick, press | the message appears as yours with the "from Habits" mark (after its FigCraft pass) |

## 9. Manifest tools (Story 9)

| Step | Do | Expect |
|---|---|---|
| (T) | manifest tool `highlight_row`; a fake tab declares it and answers | in the conversation, `S(app_highlight_row(habit_id=1))` (category = the app) → the fake tab's answer |
| (T) | the fake tab declares `evil_tool`, which is not in the manifest | `context` shows no `evil_tool` (FR-033) |
| (T) | the manifest tool with no handler registered | `{"status":"unavailable"}` at once |
| (T) | an ordinary chat with Archivist while the app is open | no app tools offered |
