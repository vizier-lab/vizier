# Research: Agent Apps

Decisions taken while planning [spec.md](spec.md). Each one resolves a question the Technical Context in [plan.md](plan.md) left open. Code references are from the 071 branch point (`1e55d27c`).

## D1. How apps are limited to the desktop edition

**Decision**: Two layers.

1. **Compile time.** A `vizier-core` Cargo feature `subapp`, listed in `full`. It is ungrouped (like `utoipa` and `telemetry`) because an app is not an agent addon, and the constitution's naming rule covers only channels, addons, providers and gateways. The feature is listed with the matching `changes`/flag entries in CI. It compiles the `apps` module, the app storage, the tools and the routes.
2. **Run time.** A new `RuntimeOptions.apps: bool`, default `false`. Only the desktop host sets it: `app/src-tauri/src/engine.rs` sets it for the window, and for its opt-in web access server too. `vizier run` never sets it.

With the switch off, the following stay off:
- the apps service is not built;
- no app tools are offered;
- `/api/v1/apps*` is not mounted;
- `GET /api/v1/utils/capabilities` reports `apps: false`.

The WebUI shows app surfaces only when that endpoint says `apps: true`.

**Rationale**: The server binary builds `full`, so a Cargo feature alone cannot keep apps off the server. The constitution also requires every flag to be in `full`. The existing desktop signal, `AuthMethod::Desktop`, is per request, but FR-001 wants apps reachable through web access too, and web-access requests carry ordinary network principals. A host-level switch is the one place that knows the edition. The `RuntimeOptions` struct (`runtime.rs:61`) already carries host choices (`roles`, `gate`). The flag keeps the code out of `--no-default-features` builds, per the constitution's optional-capability rule.

**Alternatives considered**:
- **The feature flag alone**: it doesn't exclude the server.
- **`AuthMethod::Desktop` per request**: it excludes web access, against FR-001.
- **A config key**: a server operator could turn it on, against FR-001.

## D2. The app's conversation: a new channel kind

**Decision**: `VizierChannelId::App(username, app_id)`.
- Slug: `app__{user}__{id}`.
- Web wire id: `app__<id>`, parsed by `from_web`, as projects do with `project__<id>`.
- `web_user()` returns the owner, so `is_interactive_turn`, `session_has_person`, the generative-UI offer and shares follow without extra code.

The **session** is `(driver agent, App(owner, app), topic)`. Each topic is one **conversation**, so an app has as many conversations with its driver as it has topics (FR-012), listed and opened through the existing topic routes with channel id `app__<id>`, exactly as projects do. A new conversation is a new topic id, chosen by the client as for agent chats. A different driver means a different agent id, so switching drivers starts a separate set of conversations on its own, and the earlier driver's stay with that agent. The app row remembers `last_topic` so the app reopens where the person left it.

**Waking** (FR-012b): `agents/tools/sessions/mod.rs::wakeable_channel` accepts `app__{user}__{id}` when the app's current driver is the calling agent **and** `access::belongs_to_person` holds for the app's owner (the rule spec 067 already applies to WebUI conversations). Anything else is `not_wakeable` or `other_person`. `list_sessions` lists the agent's app conversations with the app's name. The woken turn runs in the app conversation, so `apps::context::for_turn` loads the app as for any turn there. The page learns which conversation is open from the boot data and a `conversation` host message (contracts/frame-protocol.md), so an app can key state by conversation (FR-012a).

**Every place a new kind touches**, from the projects precedent:
- `schema/session.rs`: `usage_place`, `from_web`, `web_user`, `web_channel_id`, `usage_activity`, `to_slug`, plus a new `app_id()`.
- `service/conversations.rs::kind_of`: shown as `ConversationKind::App`, "App: <name>", read-only once the app is gone.
- `service/topics.rs::readable_channel`.
- `agents/tools/sessions/mod.rs`: `wakeable_channel` refuses `app__`, because a wake must not drive an app.
- The span kind in `agents/agent/mod.rs`.
- `ui_pages/verify.rs:152` and `agents/tools/ui_pages.rs::offered_in`: App is allowed like Project.
- `notifications/mod.rs`: the link is `/apps/{id}`.
- `usage/mod.rs`: the `SlugKind::App` label.
- `webui/app/lib/usage.ts`.

**Rationale**: The project chat is the proven pattern: a personal, per-person conversation bound to a second entity. Reusing the session key gives the history, steering, durable turns, client tools, notifications and usage of an ordinary chat for free. That is FR-011, "the ordinary chat".

**Alternatives considered**:
- **Reusing `HTTP(user, "app__<id>")`**: this was 040's mistake for projects, later migrated away.
- **A `Subagent`-style hidden channel**: it is not interactive and not the ordinary chat.

## D3. Where app state lives

**Decision**:
- **Rows in the main database** (SQLite only, the desktop's fixed backend): `app`, `app_version`, `app_approval`, `app_schedule`, `app_schedule_run` and `rail_order`. Schema in `storage/sqlite/app.rs::init_app_schema`. Schema in [data-model.md](data-model.md).
- **Page HTML** in the blob store, content-addressed: `apps/{app_id}/pages/{sha256}.html`.
- **App files** under `apps/{app_id}/files/{file_id}`.
- **The app database** as its own SQLite file at `{workspace}/apps/{app_id}/app.sqlite`, opened with `rusqlite` (already a bundled dependency).

There is **no PostgreSQL migration**. `AppStorage` is implemented once on `SqlStorage`. With `RuntimeOptions.apps` off, nothing calls it, and with it on the backend is always SQLite (the desktop is fixed to SQLite and local storage). `VizierRuntime::start` refuses `apps: true` on a non-SQLite backend with a clear error.

**Rationale**: Everything else in Vizier lives in the main database, which keeps backup and deletion in one place. The app database must be a separate file so the agent's SQL can never reach Vizier's own tables (FR-018), and so the owner can download it whole (FR-018b). Adding a PostgreSQL migration for a feature the server never enables would be code with no caller, against Principle I.

**Alternatives considered**:
- **App data as tables inside `vizier.db`**: agent-written SQL could reach Vizier's own tables.
- **An `ATTACH DATABASE` on the main connection**: a shared connection would leak one app's data into another's queries.

## D4. One app version is one snapshot

**Decision**: An `app_version` row holds:
- the page blob's `sha256`;
- the **manifest** as JSON: name, description, icon, functions and their scripts, page-callable functions, schedules, manifest tools, declared requirements and the **ordered list of migrations**;
- the version's origin, base version, verification result and size.

A change is either an edit set against the page (reusing `ui_pages::edits::apply_edits`), or a manifest change made by an `app_*` tool, or both. One save makes one version, and only on top of the latest (`stale_base` otherwise, as in 063).

Migrations are **append-only across versions**. A version may add migrations at the end of the list. It may never change or remove one already applied. Applying runs the new ones, in one transaction, inside the app database, and records them in that database's `_vizier_migrations` table. Only then is the version row saved. A failure leaves both untouched (spec edge case). A rollback changes `app.current_version` and nothing else (FR-017, FR-018a).

**Rationale**: A single snapshot makes rollback, history and a future export trivial. Functions are short and versioned with the page they serve, so a separate versioned store (as spec 061 has per tool) buys nothing here.

**Alternatives considered**:
- **Per-function version rows, as in spec 061**: more tables, and rollback would have to line up several histories.
- **Down-migrations**: the spec forbids undoing data changes on rollback.

## D5. Reusing the generative-UI page host

**Decision**: Reuse these parts of `ui_pages` unchanged: `edits.rs`, `ticket.rs`, `limits.rs`, the verify request and reply types, and `vizier-http/src/ui_frame.rs::document`. Generalise only the frame lookup.

The frame route `/api/v1/ui-frames/{ticket}` resolves a ticket's page id through a small `FrameSource` trait with two implementations: `UiPages` (`pg_` ids) and `Apps` (`ap_` ids, where the "page" is `app_id` at a version). This follows Principle II: a second real implementation, not branching.

The verify subject is already per session (`ui.{session}.verify`). The responder's channel gate grows to include `App`. The app's open tab is its topic socket, so it answers exactly as a chat tab does today.

The ticket stays page-and-version only. The boot data gains `app: {id, abilities}` for app frames, and the runtime advertises the new abilities only when present (`vizier.has('db')`).

**Rationale**: A page in a reply and an app page must look and behave the same (spec assumption). Sharing the host, the kit and the CSP keeps them the same by construction.

## D6. Functions: the custom-tool engine with an app owner, run where the tools are

**Decision**: App functions reuse the custom-tool *pure* parts:
- `custom_tools::validate::validate_definition`;
- `calls::parse_calls` (derived calls feed the requirement list, FR-023);
- `schema.rs` (the R6 input/output schema subset);
- `run.rs::run_with_timeout` over `python::run_script`.

They do **not** reuse the `custom_tool*` tables, whose `agent_id` is in every query.

Functions run **inside the driver's agent host**, because that is where the driver's `ToolRouter`, secrets and integrations live. There are two call paths:
1. **From a script in the app's chat or app work**: `AppSource`, a fourth `TurnToolSource` (D7), dispatches to `apps::functions::call` with the turn's own `ToolContext`.
2. **From the page with no turn** (FR-028/FR-029): a new `AgentsClient::app_function(agent, AppCall)` operation travels on `agent.{id}.cmd`, as the other lifecycle commands do.

The host's command loop builds a `ToolContext` for the app session with these fields:
- `interactive: false`
- `detached: false`
- `client: None`
- `app: Some(..)`
- the turn id `app-fn:<uuid>`, so usage is attributed

It then runs the function with the agent's current `VizierTools` snapshot.

The seam contract (`specs/048-agent-service-seam/contracts/agents-client.md`) gets the new operation added, and `FakeAgentsClient` answers it from its script.

**Rationale**: The seam already exists so that the gateway never touches a running agent's state (#163). A page-initiated call is exactly such a gateway request. Running in the host keeps secrets in the one process that holds them and redacts them through the existing `SecretRedactor`.

**Alternatives considered**:
- **Running functions in the gateway with a copied router**: it duplicates the router and leaks agent state across the seam.
- **Making the page send a chat turn**: it costs a model call, against FR-018.

## D7. The per-turn app context and tools

**Decision**: `apps::context::for_turn(deps, session, request)` mirrors `projects::context::for_turn`. For an `App` session it:
- loads the app;
- re-checks on every turn that the owner may chat with the driver;
- loads the current version's manifest;
- reads the app database's table list with row counts;
- computes the requirement report for this driver (D8).

It returns `Arc<AppTurnContext>`, stored on a new `ToolContext.app` field and set in `VizierAgent::chat`, `side_chat` and the detached copies.

`AppSource` (id `app`) offers two kinds of tools while `ctx.app` is set:
- **The authoring tools** (category `apps`):
  - `app_edit`: page edits, manifest changes, migrations; it saves and verifies a version;
  - `app_query`: SQL against the app database;
  - `app_functions`: list and describe;
  - `app_files`: list, keep, share;
  - `app_rename` and `app_set_badge`.

  Their contract is in [contracts/app-tools.md](contracts/app-tools.md).
- **The app's own functions**, under the app's category (a sanitised app name, with a `_app` suffix on a clash, the same rule as MCP keys).

The functions that need missing requirements are listed with `status: "unavailable"` and the reason (FR-025).

The **stable** facts (app name, description, how app chat works) go in a system prompt section, `system_prompt/app.rs`, which stays the same for the whole conversation. The **volatile** facts go in a `## App state` section of `context_md`, as projects do with `## Project state`, so a change never breaks the provider prompt cache: current version, tables and row counts, requirement gaps, and whether the app is open in a tab.

App tools are absent from `DREAM_TOOL_NAMES`. `offered_in` limits them to `App` sessions, never Discord or Telegram (FR-014).

## D8. The requirement check

**Decision**: A pure function, `apps::requirements::check(manifest, facts) -> Report`.

`facts` is an `AgentFacts` value with:
- the agent's tool names, read from the published view `agent.{id}.tool_names` that custom tools already publish (1 h TTL);
- its installed integration kinds and MCP server keys, from `IntegrationStorage`;
- its secret **names**, from `SecretStorage`, never values;
- its model's capabilities, from the catalogue (`ResolvedModel` caps).

The report is `Ready` or `Missing(Vec<Gap>)`. Each `Gap` holds the requirement, the reason, and a fix target: integration settings, secrets page or agent model.

The check runs in three places:
- the driver picker (`GET /apps/{id}/drivers`);
- each app turn (D7);
- before a schedule run or a page function call (FR-025, edge case "driver lacks a requirement after a change").

**Rationale**: Being pure and table-testable makes SC-005, "no false ready, no false missing", a unit test over a matrix of requirement combinations.

## D9. Schedules

**Decision**: App schedules are **not** `Task`s. A task fires into a fresh `task__` channel that is never interactive, and there is no app context. Instead, `app_schedule` rows (from the current version's manifest, kept in step when a version is saved) are ticked from the scheduler's existing minute tick (`scheduler/mod.rs:101`) through a hook, `Apps::tick(now)`.

A due schedule is one of two kinds:
- **`run: <function>`**: a page-less function call (D6) with the schedule as caller.
- **`ask: <instruction>`**: a request in the schedule's **own conversation** (topic `sched-<schedule name>`, created on the first run and reused, so a daily schedule never interrupts the person's conversation; FR-031), `VizierRequestContent::AppScheduled(String)`, with frontmatter `sender: scheduler`, `via: app_schedule`. It runs in the history arm so the driver sees the thread, as `Woken` does (spec 067).

Every run is one `app_schedule_run` row, `running → ok | failed | skipped(no_driver | missing)`. The `running` row is the overlap lock, as for task runs. A failure opens one notification per episode (`app_schedule_failed`, key `{app}:{schedule}:{episode}`), and the next success closes the episode (FR-031).

With no driver, or with missing requirements, the run is recorded as `skipped` with the reason, never as `failed` (Story 5, scenario 2).

## D10. The page abilities

**Decision**: New kit abilities, advertised in `vizier.abilities` only for app frames. All go through the existing `ask()` request/reply helper of `runtime.ts`, and the host (`SandboxedPage`) turns them into HTTP calls to `/api/v1/apps/{id}/...`. The desktop's `api` IPC command carries those calls unchanged.

| Ability | Page API | Host route | Rules |
|---|---|---|---|
| `db` | `vizier.db.query(sql, params)`, `vizier.db.exec(sql, params)`, `vizier.db.onChange(fn)` | `POST /apps/{id}/db/query` | Parameters bound, never interpolated. Time and row limits. DDL refused (migrations only, FR-018a). |
| `fn` | `vizier.fn.call(name, input)` | `POST /apps/{id}/functions/{name}/call` | Only functions approved for this driver (FR-029). Rate limit per app. |
| `chat` | `vizier.chat.send(text, {files})` | `POST /apps/{id}/chat` | Only within `SendGate`'s interaction window (the 10 s after a press or key), the gesture rule of FR-021a. Rate limit per app. |
| `files` | `vizier.files.pick()`, `vizier.files.list()`, `vizier.files.keep(id)`, `vizier.files.share(id)` | `/apps/{id}/files` | The picked file's bytes cross `postMessage` as a `Blob`; the host uploads it (the frame has no network). |
| `tools` | `vizier.tools.handle(name, fn)` | (host-local) | Answers manifest tools (D11). |

Data changes are pushed back. Every write to the app database (page, function, driver) publishes `AppEvent::DataChanged{app, tables}` on the bus (`app.{id}.events`). Each socket open on that app relays it to its tab as an `app_data_changed` frame, and the runtime calls the page's `onChange` subscribers (FR-020). The desktop's inbox pump pattern (`transport/events.rs`) gains a fourth arm.

## D11. Manifest tools as client tools

**Decision**: The host declares the current version's manifest tools as **client tools on the app's own topic only**. `webui/app/lib/clientTools.ts` gets per-topic registrations: `declaredClientTools(topicKey)` returns the global ones plus those registered for that topic. Today's registry is global, so an app's tools would leak onto every conversation's requests, which FR-033 forbids.

A call reaches the frame as a new host-to-frame message `tool.call{id, name, input}` and comes back as `tool.result`. A name with no `vizier.tools.handle` registration answers `{status: "unavailable"}` at once.

The server re-validates the declarations against the current version's manifest in `bind_client_tools` for `App` channels. A declaration not in the manifest is dropped, which is the hard guarantee of FR-033 (Story 9, scenario 4).

## D12. Messages from the page

**Decision**: `chat.send` produces an ordinary `Chat` request in the app's open conversation, with a new field `VizierRequest.app_origin: Option<AppOrigin{app_id, version}>`. The field is journaled. `stamp_on_receipt` clears it from client input, and only the `/apps/{id}/chat` route sets it.
- Frontmatter adds `via: app`.
- The WebUI draws the bubble as the person's, with a "from <app name>" mark.
- Attachments ride the existing request attachment path as session files of that conversation.

**Rationale**: The message really is the person's (FR-021a), so it must be a `Chat` and steer or queue like one, not a `ClientEvent`, whose frontmatter and rendering say "the client did this".

## D13. The WebUI

**Decisions**:
- **Rail**: `design/` `RailAgent` becomes `RailItem = {kind:'agent', …} | {kind:'app', appId, name, icon, badge?, draft?, status?}`. An app tile is a line icon in a tile, or a dashed monogram for a draft. The order moves from the per-browser `vizier.rail.order` to the server: `GET/PUT /api/v1/me/rail-order`, a `rail_order` row per person (Story 7). The old localStorage order is uploaded once and then removed. `onCreate` drives the + menu (H) in desktop mode with apps enabled. Otherwise the current `addHref` path is unchanged.
- **Chat reuse** (FR-011): `routes/chat.tsx` is split into a `ChatView({agentId, channel, topicId, header, onNavigate, chrome})` component and a thin route. The agent chat routes and the app route both mount `ChatView`. `stores/connection.ts`'s `topicKey` gains the channel, because app threads and agent chats may share a topic id space.
- **Layout**: a new design component `SplitInsets` (two insets, a keyboard-operable separator, min sizes, a double-click reset). The chat width is persisted per app per browser (`vizier.app.chatWidth.{appId}`), a per-viewer convenience. Below `lg`, the chat is a bottom `SlideOver`.
- **App host**: `SandboxedPage` takes a `ticket()` function and an `abilities` handler object as props, so the chat card (063) and the app view share it.
- **Route**: `/apps/:appId`. `AppShell.routeAgent` excludes `apps`, `viewOf` returns `app`, and the chat-route regex learns `^/apps/`.
- **Phone**: apps appear in `AgentSwitcherSheet` and `MobileHome` in the rail order (both sort by name today).
- **Icons**: the manifest names an `IconName` from `design/src/icons/map.ts`. An unknown name falls back to the monogram (FR-007).

## D14. Drafts

**Decision**: A draft is an `app` row with `current_version = 0`.
- `POST /apps` creates it with the driver and a first conversation id.
- The route discards a draft when the person navigates away if none of its conversations has a request (`DELETE /apps/{id}?if_draft_unused=1`, refused once a message exists).
- Startup sweeps drafts older than a day that have no history row (a crashed window).
- The first saved version turns a draft into an app. The driver must name it and give it an icon in that save, and `app_edit` refuses a first version without them.

## D15. Usage and notifications

**Decisions**:
- **Usage**: `UsageActivity::App` (label "Apps"), place `app:{app_id}`, through `usage_activity` on the channel. A page-initiated function call is recorded under the app session (D6), so FR-035 holds without a new scope.
- **New notification categories**: `app_schedule_failed` (once per episode), and `app_driver_lost` for the owner when a driving agent is deleted. Both link to `/apps/{id}`.

## D16. Removal and agent deletion

**Decisions**:
- **`DELETE /apps/{id}`** removes these in one storage transaction:
  - the `app`, `app_version`, `app_approval`, `app_schedule` and `app_schedule_run` rows;
  - the rail entry.

  It then deletes the blobs and the database file. Its conversations stay as conversations of their agents (FR-034), shown as "App: <name> (removed)".
- **Deleting an agent** sets `driver_agent_id = NULL` on its apps in the same transaction as the agent delete (FR-027), and records `app_driver_lost`. The agent's own conversations, the app conversations included, go with the agent as today.

## D17. Tests and the offline check

**Decisions**:
- **Unit tests**:
  - requirement check (a matrix, SC-005);
  - migration append-only rule;
  - approval matrix (SC-006);
  - gesture/rate rules (`host.ts` vitest);
  - draft rules;
  - rail-order merge;
  - frame-source lookup.
- **Integration tests**: `crates/vizier-http/tests/apps.rs` runs the routes through the network router and the dispatcher, with `RuntimeOptions.apps = true`, on a temporary workspace. `crates/vizier-core/tests/dummyplug_e2e.rs` gains app cases: a dummyplug driver runs `app_edit`, `app_query` and a function, and a schedule fires.
- **The server-side check** (SC-009): a test that builds the router with `apps: false` and asserts that no `/apps` route, app tool or capability is present.
- **Offline e2e**: [quickstart.md](quickstart.md) scripts the dummyplug steps (constitution e2e gate).
