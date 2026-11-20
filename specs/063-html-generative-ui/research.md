# Research: HTML Generative UI (063)

Each decision is recorded as Decision, Rationale and Alternatives. Code references point at the seams the work reuses.

## R1 — One built-in tool, `ui_render`, category `ui`

**Decision.**
- `ui_render` is a native `VizierTool` in `crates/vizier-core/src/agents/tools/ui_pages.rs`.
- Its category is a new `ToolCategory::Ui` (`"ui"`), the **23rd**, appended at the end of `ToolCategory::ALL`.
- `ends_turn()` is `true`.
- It is registered on `default_toolset` and gated per turn by `available(ctx)` (R9).
- It keeps the name `ui_render`. Agents, skills and memories that mention it stay right. Old json-render calls are `ClientToolCall` entries in history, so they never collide with the new entries.

**Rationale.**
- FR-001 and FR-002.
- A native tool gets storage, fencing and permissions from core for free.
- Reusing the name avoids a rename that would break what agents already learned.

**Alternatives.**
- Keep it a client tool. This was rejected by the person: pages must exist with no tab open.
- A new name such as `ui_page`. This churns prompts and memories for no gain.

## R2 — The reference is read through `describe_tool_function`, not a second tool

**Decision.**
- `VizierTool` gains one optional method, `fn guide(&self) -> Option<&'static str>`, which defaults to `None`.
- `describe_tool_function` adds the guide to the doc it returns, as a `guide` field.
- `ui_render`'s guide is the kit reference: blocks, attributes, examples, bridge abilities, libraries and limits.
- The guide text lives in core as `crates/vizier-core/assets/ui-kit/REFERENCE.md` (`include_str!`).
- The tool's own description stays short and fixed. It says: "read `describe_tool_function("ui_render")` once per conversation before the first page".

**Rationale.**
- FR-004: the reference is on demand and never in the system prompt.
- The person wants exactly one UI tool. `ui_components` was a second callable tool, and it is retired (R10).
- `describe_tool_function` is already how a script learns a tool.

**Alternatives.**
- A `reference: true` mode of `ui_render`. Turn-ending is decided per tool (`bridge.rs:215`), so a reference call would end the turn.
- Putting the reference in the description. That costs every turn and breaks the 2 KB discipline.

## R3 — Three input shapes; refusals and problems are errors

**Decision.**
`ui_render` takes exactly one of these shapes (see `contracts/ui-render-tool.md`):

| Shape | Arguments | Result |
|---|---|---|
| New page | `{title, html}` | creates page version 1 |
| Full replacement | `{page, base_version, html, title?}` | creates version N+1 |
| Edit set | `{page, base_version, edits: [{find, replace}, …]}` | creates version N+1 |

- **How edits apply.** Edits apply in order, each to the text the previous one produced. A `find` must occur exactly once at that point, and an empty `find` is refused. The whole set is applied to a copy, and nothing is saved unless every edit applies (FR-008).
- **Refusals** (bad shape, unknown page, another conversation's page, stale `base_version`, an edit that does not match, over the size limit) and **verification problems** are returned as `Err(VizierError)`. The message is JSON text that names the page, the version and each problem. This is the precedent of `ask_question` (`questions/tool_question.rs`). A turn-ending tool only ends the turn on `Ok` (`ExecutionReport::ended_turn`), so an `Err` keeps the turn running (FR-003, FR-013), and the script can `try/except` it.
- **On verification problems the version is already saved** (FR-020). The error carries `{page, version}`, so the next call can be an edit based on it.

**Rationale.**
- Exact find/replace is what agents already do well with `edit_file`.
- Applying edits in order matches how a person describes a change.
- `Ok` for success and `Err` for "keep going" is the only signal the turn-ending rule reads.

**Alternatives.**
- A `status` result for refusals. It would be `Ok` and would end the turn on a refusal.
- Unified diffs. Models produce them less reliably than find/replace.
- Line-number edits. They are fragile across versions.

## R4 — Storage: metadata in SQL, HTML in the blob store

**Decision.** There are two tables, defined in `contracts/storage.md` and `data-model.md`:
- `ui_page` holds the identity, owner conversation, title, latest version, saved state and `state_seq`, and owner fence columns.
- `ui_page_version` holds the number, size, sha256, origin (`full` | `edits`), base version, created time and verification outcome.

The HTML bytes live in `BlobStore` at `ui-pages/{agent_id}/{page_id}/{sha256}.html`, named by content so a stale edit set racing the winner can never overwrite its bytes.
- A new `UiPageStorage` trait is added to `VizierStorageProvider`.
- It is implemented once on `SqlStorage`, for SQLite (`init_ui_page_schema`) and PostgreSQL (migration `0011_ui_pages.sql`).
- Every write is fenced on `agent:{id}` (`check_fence` in its own transaction).
- Creating a version is one transaction: `latest_version = base` → `base + 1`, guarded so two racing edit sets cannot both win (edge case "two edit sets").
- The blob is written first, under its final key. A row that never commits leaves an orphan blob, which the next delete of the page removes, because deletion lists by prefix.
- `delete_session` and `delete_agent` remove the rows. The blobs are removed by prefix `ui-pages/{agent}/{page}/`, after the rows are gone (FR-012).

**Rationale.**
- "Database rows hold file metadata … never object content" (CLAUDE.md, Storage).
- The blob store already serves local and S3 storage.
- The fence keeps a stale owner from creating versions after the agent moves.

**Alternatives.**
- HTML in a SQL text column. That breaks the storage rule, and 1 MiB rows bloat backups.
- Reusing the artifact tables. Artifacts are promoted files with their own permissions and gallery, and pages are conversation-scoped. Phase 2 may promote a page to a subapp, and that is where the two can meet.

## R5 — History entry and how the WebUI learns of a version

**Decision.**
- `SessionHistoryContent::UiPage { page_id, version, title }` is saved through `agents::steps::record_step` when a version is created. The same pattern saves `QuestionSet` (`questions/submit.rs::record_set`).
- Its verification outcome changes after the insert. It is filled on every history read from `ui_page_version`, as `fill_client_calls` does (`fill_ui_pages`).
- `history_entries_to_messages` skips it: the model already has the tool's result.
- `record_step` already publishes the entry as `SessionFrame::HistoryEntry`, so open tabs draw it with no new frame. The verification outcome, which comes later, is published as `SessionFrame::UiPageVerified { page_id, version, verification }`.

**Rationale.**
- FR-006: each reply keeps its version.
- The entry is what the WebUI draws, and a failed version shows in the turn's trail while the turn continues (Story 3: the person sees the fixed version as the reply).

## R6 — Verification over the bus, with "no tab" detected by `NoResponders`

**Decision.**
- Every chat socket that watches a WebUI or project session subscribes, as a responder, to `ui.verify.{session_key}` on the Fleet bus (`vizier-schema` `Messaging`). It does this when it opens and drops the subscription when it closes.
- After saving a version, `ui_render` makes one bus **request** to that subject with `{request_id, page_id, version}`, with a deadline of `verify_timeout` (10 s).
- **No responder.** The bus answers `NoResponders` at once. The result is `verified: false`, and the turn ends with no wait (FR-019).
- **A responder.** The socket handler sends the browser `ui_verify`. The browser mounts the version in a hidden, off-screen verifier (R7), observes it for `observe` (2 s), and answers `ui_verify_report {request_id, problems}`. The handler replies on the bus.
- **Several tabs.** A bus request takes the first reply (FR-017's "exactly one report"). Other tabs that also verify are ignored.
- **The deadline passes, or the socket closes mid-check.** The result is `verified: false` (FR-019).
- The outcome is written to `ui_page_version.verification` (fenced) and published as `UiPageVerified`.

**Rationale.**
- The bus is the Fleet seam (#162). A gateway on another node can answer.
- `NoResponders` *is* the presence check, so no watcher registry is needed.

**Alternatives.**
- An in-process `oneshot` map, like `Questions::open_ask`. It is simpler but process-local, and it would need a separate presence check.
- Verifying only when the entry is drawn visibly. Timing then depends on scroll and focus, and a failed draft would flash in front of the person.

**Checked.**
- `InProcess::request` (`vizier-schema/src/inprocess.rs`) routes to every subscriber and returns the first reply.
- It returns `BusError::NoResponders` when the route reaches nobody, and `Timeout` at the deadline.
- Every open tab therefore verifies and the first report wins. Each tab already has the page open, so the extra work is small.

## R7 — One sandbox host for chat and verification

**Decision.**
- One WebUI component, `screens/chat/client/SandboxedPage.tsx`, built on today's `InteractivePage.tsx`, and its document builder `lib/uiPage/document.ts`, which supersedes `lib/pageDocument.ts` for new pages.
- Isolation: `sandbox="allow-scripts"` (no `allow-same-origin`, so the page has an opaque origin), plus the CSP in R8.
- In-frame hooks report `error`, `unhandledrejection`, `console.error`, `securitypolicyviolation` and kit attribute problems over `postMessage`.
- The verifier mode mounts the same host off-screen (fixed 1024×768, `visibility: hidden`, outside the viewport). It collects problems for `observe`, and reports **unresponsive** if the frame's 250 ms heartbeat stops.
- Error indicator (FR-021): a problem raised after mount in chat mode lights an indicator in the frame header. Hovering or pressing it lists the problems.

**Hangs (FR-016, FR-016a).** A sandboxed, opaque-origin frame runs in its own process only where the browser isolates sandboxed frames. Chromium does this by default (Chrome, Edge, WebView2 on Windows), and WebKit does not (Safari, macOS WKWebView and Linux WebKitGTK in the desktop app). On WebKit, a page that loops forever can freeze the tab. This is the reason `generativeUi/logic/` runs logic in a worker inside the frame.

Only one task that runs too long freezes a page, and a page that yields between tasks never does: `requestAnimationFrame`, timers and awaited events all yield. Two kinds of task are risky:
- an endless synchronous loop, which is rare and usually a bug in a draft;
- a heavy but finite computation, such as a chess search in a click handler. This is the realistic case.

Decision (2026-11-16): move heavy work off the main thread by guidance and a helper, not by rewriting page code.
- **Background helper.** Bridge v1 gets a `worker` ability: `vizier.worker(fn | "#script-id")` returns `{run(input, {timeoutMs}), terminate()}`.
  - It builds a dedicated worker from a `blob:` URL (the CSP already has `worker-src blob:`, and a blob worker inherits `connect-src 'none'`).
  - `run` rejects with `timeout` and terminates the worker when its limit passes. The default is 10 s, and the maximum is 60 s.
  - `<script type="vz-worker" id="...">` holds a worker's source without running it on the page.
- **The guide** says, in its rules section: drive animation and game loops with `requestAnimationFrame`; put search, AI, simulation steps and anything else that can run longer than about 50 ms in `vizier.worker`. It includes one example (a minimax opponent in a worker).
- **Verification** reports `unresponsive` when the frame's heartbeat stops, wherever the parent survives.
- **Crash-loop marker.** Before mounting a version, the host writes `vz.mounting = {page, version}` to `localStorage`, and clears it after three heartbeats. If a reload finds the marker set, that version froze the tab: it is drawn "paused · Run anyway". The next turn's per-message context gets one line naming it, so the agent can move the work to a worker.

**Rejected for now:**
- **Instrumenting loops** (CodePen-style rewriting at mount, which needs a JS parser dependency).
- **A Tauri watchdog** that reloads a stuck webview.

Both can follow if freezes happen in practice.

## R8 — CSP and the kit's origin

**Decision.** Every page document starts with this CSP:

```
default-src 'none';
script-src 'unsafe-inline' {KIT};
style-src 'unsafe-inline' {KIT};
font-src data: {KIT};
img-src data: blob: https:;
media-src data: blob: https:;
connect-src 'none'; frame-src 'none'; worker-src blob:; form-action 'none'; base-uri 'none'
```

- `{KIT}` is the WebUI's own origin plus `/ui-kit/`, computed at runtime (`new URL('/ui-kit/', location.href)`). In the desktop app this is the WebView's origin.
- This meets FR-015 (scripts cannot reach the network) and FR-015a (`https` media only; plain `http` is refused and shows up as a `securitypolicyviolation` in verification).
- `worker-src blob:` lets a page start a Web Worker from its own code, which a game may want. Workers inherit `connect-src 'none'`.
- Library modules are loaded from an opaque origin, so their responses need `Access-Control-Allow-Origin: *`. `vizier-http`'s WebUI asset handler adds it for `/ui-kit/*` only, and the Tauri asset path does the same. The files are public, static and hashed, with no credentials.

**Accepted risk.** `img-src https:` lets a script put data in an image URL and reveals the person's IP to that host. This was decided on 2026-11-16 (spec Assumptions).

**Found while building (2026-11-16): the desktop app's CSP is inherited by `srcdoc` frames.**
- A `srcdoc` (and `blob:`/`data:`) document inherits the embedding page's CSP.
- The desktop app's CSP (`app/src-tauri/tauri.conf.json`) has no `'unsafe-inline'` for scripts.
- A probe with that exact policy on the parent blocked the inline script of a sandboxed `srcdoc` frame. With no parent CSP (the web build, where `vizier-http` sends none), the same frame runs.

So in the desktop app, inline page scripts don't run as built today. That likely includes the existing `render_interactive_ui` pages and json-render's `logic` runner.

**Options:**
- **(a) Serve each page frame from a URL with its own CSP header.** A navigated document does not inherit. In the desktop app this would be the `vizier-asset:` scheme, added to `frame-src`.
- **(b) Add `'unsafe-inline'` to the app's `script-src`.** This weakens the WebUI's own XSS protection, and Tauri's injected hashes would make it ignored anyway.

**Decision (2026-11-16): option (a).** Named R7a here, built and checked:

- **The frame is a document the server builds** (`vizier-http/src/ui_frame.rs`), served at the public route `/api/v1/ui-frames/{ticket}` with `FRAME_CSP` as a **header**.
  - The header carries `sandbox allow-scripts`, so the document has an opaque origin whatever the `<iframe>` says. The host sets no `sandbox` attribute.
  - In the browser pane, sandboxed frames loaded by URL from `127.0.0.1` ran nothing, while the header-only sandbox ran with `self.origin === "null"` and no cookie access.
- **The ticket** (`vizier-core/src/ui_pages/ticket.rs`) is `{page}.{version}.{expires}.{HMAC}` under a per-process key, valid for 10 minutes.
  - It is minted by `GET …/pages/{id}/versions/{n}/frame` after the read check.
  - It carries no credential, because the page can read its own URL.
  - Tickets are per process, so a gateway in a Fleet only opens the tickets it minted. That is fine while a tab's requests go to one gateway; a shared key is a Fleet follow-up.
- **The frame fetches nothing.** The kit stylesheet, the runtime (`webui/app/kit/runtime.ts`), the element script (only with a `<vz-` tag) and libraries (only when imported) are inlined.
  - A library becomes a blob URL behind an import map, so `three` is bundled into one module by the kit build.
  - The boot data (page id, version, saved state) is inlined as JSON with `<` escaped.
- **The raw version route** answers `text/plain` with `nosniff` and the same policy, so a page never runs as a document of the WebUI's origin.
- **Desktop app:**
  - the asset scheme forwards `content-security-policy`, `cache-control` and `referrer-policy`;
  - `frame-src` allows `vizier-asset:` and `http://vizier-asset.localhost`;
  - `transport.frameUrl` builds the URL with no token.
- **Checked:**
  - `tests/ui_frames.rs` covers the routes, headers and boot data through the dispatcher and the public router.
  - The served document, under the desktop app's own CSP on the parent, posted `ready`, its height and heartbeats with no problems.
  - **Still to check in the real app (`just app-dev`):** WebView2 and WebKit loading a `vizier-asset` frame.

## R9 — Where the tool is offered

**Decision.** `available(ctx)` returns `true` only when both of these hold:
- `ctx.session.1` is `VizierChannelId::HTTP(..)` or `VizierChannelId::Project(..)`;
- `!ctx.detached`.

Everything else falls out of the channel kind:

| Excluded | Why |
|---|---|
| Tasks, dreams, subagent workers, Discord, Telegram, Workspaces | their channel kinds are different |
| `/btw` | it never reaches tools |
| Detached scripts | the `detached` check |

- There is no dependence on open sockets (FR-032). `SANDBOX.md`'s category line for `ui` is therefore stable for a conversation.
- `ui_render` is not in `DREAM_TOOL_NAMES`.

**Alternatives.** Gating on `ctx.interactive`. It changes with the request kind (for example a delivery turn), which would make the listing vary within a conversation.

## R10 — Retiring the two old tools

**Decision.**

WebUI:
- Remove the `components` and `render` client registrations.
- Re-register both old renderers with `declare: false`, so saved calls still draw with their state: `render` → `GenerativeUi.tsx`, and `render_interactive_ui` → `InteractivePage.tsx`.
- `lib/generativeUi/` stays as read-only rendering code. Its validation, reference and recipes go once nothing imports them, and the renderer stays until #82.

Server:
- `ui_components` and `render_interactive_ui` join `RETIRED_TOOL_NAMES`, with guidance naming `ui_render`. `ui_render` itself is the live name.
- A stale tab that still declares the client category `ui` (`components`, `render`): `client::declare::validate` drops those two declarations with a `warn!` instead of renaming them `ui_client_*`, so an old bundle cannot add a second UI tool.

Both shims go on the #82 checklist (FR-037).

## R11 — The kit

**Decision.** A separate build entry in `webui`, `app/kit/`, produces `build/client/ui-kit/`:

- **`kit-{hash}.css`**:
  - `--vz-*` tokens for light and dark;
  - base element styles (body, headings, links, `button`, inputs, `table`, `code`, scrollbars);
  - a few layout classes (`vz-stack`, `vz-row`, `vz-grid`).
- **`kit-{hash}.js`**, a classic script. It defines the custom elements `<vz-card>`, `<vz-stat>`, `<vz-table>`, `<vz-chart>`, `<vz-tabs>`, `<vz-field>` (text, number, select, checkbox, slider), `<vz-badge>`, `<vz-callout>`, `<vz-progress>` and `<vz-image>` (a session file by name).
  - They are React 19 roots inside the frame, rendering the same `@vizier/design` components the WebUI uses, so the look stays single-sourced (FR-024, Principle II).
  - Attributes are parsed with the zod schemas already in `lib/generativeUi/catalog/`, moved to `app/kit/schema/`.
  - An invalid attribute draws an inline problem box and reports a `kit` problem.
- **`lib/three-{version}.js`** (ES module, from the existing `three` dependency). An import map in each page document maps `"three"` to it (FR-025). Charts come from `<vz-chart>`. The library list is `app/kit/libraries.ts`, which is data.
- **Theme.** The host posts the resolved token values in its first message and again on every `useAppearanceKey` change. The kit sets them on `:root` without a reload (FR-023).
- **Reference sync (FR-026).** `scripts/ui-kit-reference.test.mjs`, in the workspace and run by `just test-scripts`, reads the kit's exported element manifest (`app/kit/manifest.ts`, built to `ui-kit/manifest.json`) and `assets/ui-kit/REFERENCE.md`. It fails on any element, attribute or library present in one and missing from the other.

**Cost.** React and the design components are about 60–80 KB gzipped for the frame. The file is hashed and cached, and it is loaded only when a page uses a `vz-*` tag or the kit script is asked for. The CSS is always included and is small.

**Alternatives.**
- Vanilla custom elements written fresh. Rejected: it would be a second implementation of every design component (Principle II), and it would drift.
- Preact. It is a second React-like dependency.

## R12 — Bridge v1

**Decision.** In-frame `window.vizier`, specified in `contracts/bridge.md`:

```
{ version: 1, abilities: ["state", "send", "files"],
  state: { get(), set(value), subscribe(fn) },
  send(text, data?),
  files: { url(name) } }
```

- **State.** It is coalesced to one save per 500 ms (FR-027: at most twice a second).
- **Send.** Allowed within 10 s of a trusted interaction and at most once a second, enforced by the host. These are the rules from `InteractivePage.tsx`, kept.
- **Files.** `files.url(name)` resolves through the host: the host fetches the session file and returns a `data:` URL, because an opaque origin can use neither the token nor a blob URL from the parent.
- **Later abilities.** A page asks `vizier.has("tools")` before using an ability. Phase 2 adds abilities to the list without changing v1 (FR-030).
- **Sending a message.** It is sent as today's `client_event` (`VizierRequestContent::ClientEvent`), with `source: {page_id, version}` instead of a client call id. The request gains an optional `page` source field.
- **Saving state.** State travels over the socket as an inbound `ui_page_state` frame (mirroring `client_call_state`), rate-limited by a `RateLimiter` on `HTTPState`. It is stored on `ui_page` with `state_seq`, fanned to other tabs as `SessionFrame::UiPageState`, and a new version starts from the page's state (Story 5.4).

## R13 — Presentation

**Decision.**
- **In the reply.** The frame header shows the title, `v{n}`, an "unchecked" mark when `verified: false`, the error indicator, full screen (the existing `useChatAreaFullscreen`) and Download (FR-033 to FR-035).
- **Failed versions.** Within a turn, versions whose verification failed render collapsed in the trail as "v1 · 2 problems". The final version renders as the reply.
- **Download.** `standaloneDocument` inlines the kit CSS and JS and the latest state, so the file works alone (FR-034). Libraries are inlined only if the page imports them.
- **Mockups.** Per the design-first rule, the first UI task is `specs/063-html-generative-ui/mockups/page-frame.html`, reviewed before any UI code. It shows the header states, the collapsed failed version, the error popover and the phone width. It is an HTML mockup because the FigCraft MCP was unreachable in this session.

## R14 — End-to-end checks

**Decision.**
- **Dummyplug** drives `execute_python` calls of `ui_render` through the real loop (quickstart).
- **`scripts/ui-pages-e2e.mjs`** opens a topic socket as a fake tab. It answers `ui_verify` with scripted reports (none, problems, silence) and asserts:
  - the tool's result for each case;
  - the saved versions;
  - the history entries;
  - the `UiPageVerified` frames.
- **Real-browser checks** use the preview tools: isolation suite, theme switch, error indicator and phone width.
- **Unit tests in core:**
  - edit application;
  - the version race;
  - availability by channel kind;
  - one fence test per write in `storage/sqlite/fence_tests.rs`.
- **Vitest in webui:**
  - the document builder;
  - CSP;
  - the bridge;
  - kit attribute parsing.

## R15 — Limits

| Limit | Value | Where |
|---|---|---|
| Version size | 1 MiB | `ui_pages::limits::MAX_VERSION_BYTES` |
| Edits per set | 100 | same |
| Title | 120 chars | same |
| Verification observe | 2 s | sent in `ui_verify` |
| Verification deadline | 10 s | tool side |
| State size | `MAX_STATE_BYTES` | the same constant `client_call_state` uses (`vizier-http/src/api/v1/agents/channel.rs`) |
| State saves | ≤ 2/s per page | host coalescing + server limiter |
