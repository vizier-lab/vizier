# Quickstart: WebUI Rewrite

Checks are grouped by user story. Server checks run offline against an agent `plug` on the `dummyplug` provider (constitution e2e gate), using its protocol (`tools` → tool name → JSON request; after one tool call, the reply is `**Tool result** (\`<tool>\`):\n<output>`). WebUI checks are in the Playwright suite `webui/e2e/` (`npm run e2e`, with `VIZIER_URL` and an admin login). Anything marked **manual** is a side-by-side review against the design.

`HOLD(n)` holds a turn open, as in spec 023:

```text
HOLD(n) = {"tool":"execute_python","arguments":{"intent":"hold","code":"shell_exec(command=\"sleep n\")"}}
```

## §0 Setup from a clean clone (US2, SC-006, SC-007)

```sh
git clone --recurse-submodules <vizier> && cd vizier
just install          # submodule update --init, npm i in design/ and webui/, cargo fetch
just run              # builds webui/, embeds webui/build/client, starts on :9999
```

1. **Expect**: `http://localhost:9999` serves the new Login page (the dot-board V and the wordmark). `git submodule status` lists `design` and `webui` at their pinned commits.
2. `grep -rn "legacy" build.rs Cargo.toml src/channels/http/webui .github/workflows Dockerfile justfile` → no hits.
3. `mv legacy /tmp/legacy-out && cargo build && just run`. **Expect**: it builds, and every page loads. Move it back.
4. `cargo package --list | grep webui/build/client/index.html` → present. `cargo package --list | grep -c '^legacy/'` → 0.

## §1 Onboarding and sign-in (US5)

1. Start on an empty data dir (`--data-dir /tmp/vz-empty`). Open `/`. **Expect**: redirect to `/onboarding`, and `GET /api/v1/status` returns `setup_required: true` with the running version shown in the footer.
2. Create an admin. On the provider step, **expect** the Ollama hint ("No API key? Ollama runs open models…") and no mention of dummyplug.
3. Create agent `plug` on dummyplug (an engineering step, through the provider select's dev entries or the API). Send `tools`. **Expect** a reply listing the four sandbox tools. Time from step 1 to here: under 5 minutes (SC-003).
4. Sign out and back in. **Expect**: Login, then Home.

## §2 Chat (US1)

1. Send `tools`. **Expect**: Send turns into Stop at `thinking_start`, and the reply renders through Prose with the footer showing model, tokens in and out, cost and the reaction button.
2. Send `HOLD(6)`. While it runs: **expect** the live trail with one running `execute_python` call (live dot). When it ends, **expect** the trail folded into "Worked 6.xs · 1 python run".
3. Send `HOLD(8)`, then type "second" and send. **Expect** "second" shown as queued (dashed). When the first turn ends, it sends.
4. Send `HOLD(30)`, then press Stop. **Expect**: the StoppedNote "Stopped by you at …", the queue dropped, and the history's `ToolCall` sealed with a `Stopped:` result (spec 023).
5. Send `HOLD(8)`, then reload the page mid-turn. **Expect** the turn so far within 2 s, and the rest of it streaming in (SC-005). Open a second tab: same.
6. Kill the network (devtools offline) for 5 s. **Expect** the Offline banner, then reconnect and no duplicated steps.
7. Type `/`. **Expect** the SkillMenu with built-in commands (`/checkpoint`, `/lobotomy`, `/skills`, …), filtering as you type; Tab completes.
8. Send `/checkpoint`. **Expect** a CheckpointDivider that opens the handover summary.
9. Configure a provider that fails (an invalid key). Send a message. **Expect** the provider-unavailable Notice with "Open Providers" (FR-008).
10. **Manual**: every state on the "Chat — states and flow" sheet matches, in both themes.

## §3 Turn internals (US1 scenarios 7–9, FR-018e/f/g)

1. Seed memory: `{"tool":"memory_write",…}` a document in bundle `infra` about "qdrant". Set `auto_context.chat_passages=5` and threshold `0.0` on `plug`.
2. Send "qdrant". **Expect**: `GET …/history` has an entry with `content_type: "recall"`, `origin: "auto"`, and passages naming `infra` and the document. The Recalled memory panel lists it with its score, and clicking it opens Memory at that document.
3. Send a `memory_search` request through `execute_python`. **Expect** a `recall` entry with `origin: "search"`, and the trail summary counting the passages.
4. Generate a file in the topic (e.g. a `write_session_file` tool call; check the name with `tools`). **Expect**: a `session_file` frame on the open socket, the Session files panel showing it with the sender `plug`, and `GET …/topic/{topic}/files` listing it.
5. Open a CSV of over 1 MB in the preview. **Expect**: the first rows render as a sortable Table, a "showing the first 256 KB" note, and the network request has `Range` and got `206`.
6. **Expect**: the context ring shows `current_context_size / context_window` from the latest reply, and the hover tooltip shows the exact numbers.
7. Reload. **Expect**: the recalled passages, files and ring are all still there.

## §4 Home (US6, FR-018/018a/018b/019)

1. With only `plug`: **expect** the one-agent ActivityGrid (single full-width row) and the summary line with today's counts by kind, matching `GET /overview/activity` → `today_by_kind`.
2. Create 8 dummyplug agents (`plug1`…`plug8`) via the API, and send a different number of messages to each. **Expect**: the "All 9" row plus the top 5, the panel the same height as with one agent, and hovering a dot shows "HH:00 · N turns".
3. Send `HOLD(10)` to `plug3`. **Expect** its newest dot emerald while it runs, and only then.
4. Stop using the WebUI for 31 minutes (or set `previous_visit_at` directly in the DB for the test). Then run a task on `plug` and trigger a dream. Reload Home. **Expect** "Since you were away" listing the task result and the dream, newest first, and "Load older" after 20.
5. Make `plug2` fail to start (set its model to an Ollama model that doesn't exist). **Expect** a pinned failed item at the top of the feed, and the agent card showing "failed · retry in …".

## §5 Dreams (US3 scenario 6, FR-018c/018d)

1. Have a conversation on `plug`, then `POST /agents/plug/dream/trigger`. During consolidation, the dummyplug model can't write memory on its own, so drive it: queue the dream's writes through the dream session (or seed revisions with `dream_cycle_id` in a test DB).
2. **Expect**: `memory_revision.dream_cycle_id` and `core_revision.dream_cycle_id` are set for the cycle's writes. `GET /agents/plug/dream/nights` has tonight with `memories = written + revised + removed` and `core = true` when CORE changed.
3. **Expect**: tonight's NightStrip dot is sized for that count, with a ring when CORE changed, and live while the cycle runs. Clicking it scrolls to that cycle's DreamJournalEntry, whose change lines are wrote / revised / removed / core, each linking to the document or revision.
4. Legacy rows: insert a `memory_revision` with `trigger='dream'` and no cycle id, inside an old cycle's journal window. **Expect** it counted on that night. Insert one outside every window. **Expect** `unattributed: 1`, and no night changed.

## §6 Lists at scale (US6, FR-022, SC-004)

1. Seed 50 agents, 300 tasks on `plug`, 1,000 bundles, 600 conversations and 1,000 roles (`scripts/seed-scale.mjs`, part of this feature).
2. `GET /agents/plug/tasks` with no params → the same JSON shape as on `master`. `?limit=25&filter=active&sort=next_run` → `{items: 25, total, counts: {all, active, last_run_failed}}`.
3. In the WebUI, open Tasks, Skills, Users, Roles, MCP, Memory › All bundles and the conversation picker. **Expect**: first content within 2 s each, and no response carrying more than one page (check the network panel).
4. `scripts/client-tools-e2e.mjs` and `scripts/durable-turns-e2e.mjs` pass unchanged (SC-009).

## §7 Day-to-day and config pages (US3, US4)

1. Core: edit, then save. **Expect** a new revision in the history list. Open the previous revision, restore it, and see the diff. Open Core in two tabs, save in tab A, then save in tab B. **Expect** `409 stale_revision` and the "Reload theirs / Keep mine" choice (FR-025).
2. Skills: edit a skill in rich, markdown and split modes. **Expect** the markdown round-trip to be lossless (save, reload, compare). Leave with unsaved changes, come back, and get the offer to restore the draft.
3. Tasks: create a one-time task with the DateTimeField. **Expect** it to run, the row's LATEST RUN to show the outcome, and its run conversation to open.
4. New agent: go through all 4 steps. **Expect** nothing persisted until "Create". Config: change the model and save. **Expect** the rebuild warning beforehand.
5. Delete an agent. **Expect** the typed-ID confirmation listing what is removed.
6. Integrations: add an MCP server (`npx @modelcontextprotocol/server-filesystem`), switch on a grant, and see the Activity entry.

## §8 Themes, mobile, keyboard (US7, SC-008)

1. **Manual**: each screen in dark and light, against the design frames.
2. At 390 px wide: Home and Chat follow the Mobile designs, with no horizontal scroll (Playwright asserts `scrollWidth <= clientWidth` on every route).
3. Tab through Login, Chat, Tasks and Roles with the keyboard only. **Expect** a visible focus ring on every control, and the Picker, SkillMenu, Dialog and FilePreview shortcuts working.
4. `npx playwright test e2e/a11y.spec.ts`: axe has no serious or critical violations on any route, in either theme.

## §9 Legacy links (FR-016)

`/plug/settings` → `/plug/config`; `/settings` → `/settings/profile`; `/plug/chat/<old topic>` opens that topic; `/nope` → `/`.
