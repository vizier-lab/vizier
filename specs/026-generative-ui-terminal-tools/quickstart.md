# Quickstart: Generative UI and Turn-Ending Tools

Everything below runs against a local binary and an agent on the offline `dummyplug` provider unless a step is marked **[live model]**. Setup:

```sh
just install && just run           # dev.vizier.yaml; create an agent "plug" on the dummyplug provider in the WebUI
cd webui && npm run typecheck && npm test          # unit tests, recipe validation
```

dummyplug steps are written as *send → expect*. To run a script as the agent, send `execute_python`, read the sample, then send the JSON (`code` and `summary` are required).

## 1. The `ends_turn` flag, protocol level (no WebUI)

Script: `VIZIER_TOKEN=<jwt> AGENT=plug node scripts/client-tools-e2e.mjs` (new case `ends_turn`, run with the 019 cases).

1. Connect a socket and send a request declaring `{"name":"finish","category":"probe","description":"probe","input_schema":{"type":"object"},"ends_turn":true}` and `{"name":"look","category":"probe",…}` (no flag).
2. Send `tools` → expect `probe_finish` listed, and `describe` text for it ending with the **ends-turn notice** exactly once; `probe_look` has no notice.
3. Send `execute_python` with `code: "probe_look()"` → the client answers; expect a **second** model round (the dummyplug echoes the report) and the turn ending normally. *(control)*
4. Send `execute_python` with `code: "probe_finish()"`, client answers with a result → expect: the reply frame arrives right after the client's answer; `usage_event` rows for the turn have **one** answered completion (no closing round); the saved reply has `stats.ended_by_tool == "probe_finish"`; history ends with the `execute_python` result.
5. Same, but the client answers with an **error** → expect the turn continues (second round), no `ended_by_tool`.
6. Same, `code: "probe_finish()\nraise ValueError('x')"` → report `ok:false`; the turn continues.
7. Same, with the declaration `"deferrable": true` and the socket closed before the call → the script gets `{"status":"deferred"}`; the turn continues.
8. Send a declaration with `"ends_turn": "yes"` → `client_error` `invalid_client_tools`, rule `ends_turn_type`.
9. Restart vizier while step 4's turn is `running` right after the client answered (kill after the result is saved, before the reply) → on restart the turn is resumed and **ends** without a new completion.
10. A request declaring no `ends_turn` anywhere behaves exactly as the 019 quickstart (run the existing cases unchanged).

## 2. Generative UI in the WebUI (dummyplug)

Open the WebUI, pick the `plug` agent, a fresh topic.

1. Send `tools` → expect `ui_components` and `ui_render` listed in the `ui` category; **no** `ui_render_interactive_ui`.
2. Send `ui_components` then the sample JSON → expect the component index (all groups, with Chart, Table, Heightfield…).
3. Send `ui_components` with `{"recipe":"dashboard"}` → expect a full spec.
4. Send `ui_render` then this JSON (a tiny interface):
   ```json
   {"title":"Demo","spec":{"root":"r","state":{"n":0},"elements":{
     "r":{"type":"Stack","children":["s","b","t"]},
     "s":{"type":"Stat","props":{"label":"Clicks","value":{"$state":"/n"}}},
     "b":{"type":"Button","props":{"label":"Add"},"on":{"press":{"action":"setState","params":{"statePath":"/n","value":{"$computed":"add","args":{"a":{"$state":"/n"},"b":1}}}}}},
     "t":{"type":"Text","props":{"text":"Local only"}}}}}
   ```
   Expect: the turn ends immediately after the call (no closing message round); the interface appears in the reply bubble in the active theme; pressing **Add** increments locally and starts no turn.
5. Reload the topic → the interface is back with its last count. Open a second tab → same.
6. Send `ui_render` with an invalid spec (`"type":"PieChart"`, a dangling child id, `"variant":"big"`) → expect an error listing all three problems with valid options; the turn **continues** (a closing model round happens) and nothing is drawn.
7. Send `ui_render` with a 300 KB spec → `too_large`, turn continues.
8. Press a `sendMessage` button (use the `form` recipe via `ui_components` → paste into `ui_render`): the text appears as the person's message, `data` reaches the agent's context, and a second press within one second is ignored.
9. Switch the app theme with an interface on screen → it re-themes without reload.
10. Open a conversation that has a page saved by the old tool (copy an old topic db, or use one from the 019 quickstart) → it still draws and keeps its state.

### 2b. Links and media

1. In the topic, attach `photo.png`, `clip.mp3`, `movie.mp4`, `report.pdf` (or have the agent generate them).
2. `ui_render` with a `Gallery`, an `Audio`, a `Video`, a `FileCard` and an `Image` naming those files → they display; audio and video do **not** autoplay; `FileCard` opens the preview dialog and downloads; seek and speed work.
3. `ui_render` with `"file": "nope.png"` → error naming `nope.png` and listing the four available files; the turn continues.
4. `ui_render` with `Image` `src: "https://example.com/x.png"` → refused (only `data:image/` or `file`).
5. Generate an image with the image tool and show it in the same script → it appears after the turn ends.
6. Delete the file from the session files panel, reload → "file not available" appears in that spot only.
7. `Link` with `https://example.com` and text "Docs" → opens in a new tab with no opener; text "paypal.com" with href `https://evil.test` → `(evil.test)` shown beside it; `javascript:alert(1)`, `data:text/html,…`, `//host` → refused with the allowed schemes listed.

## 3. Flexibility: chess and low-poly terrain

### 3a. Recipes (dummyplug, deterministic)

1. `ui_components` with `{"recipe":"board_game"}` → send the returned `spec` to `ui_render`. Expect an 8×8 board; press a pawn: its legal squares highlight; press a highlighted square: the pawn moves, the turn marker flips; no agent turn started (check the topic's history). Press an empty square with nothing selected: nothing happens.
2. `ui_components` with `{"recipe":"scene_3d"}` → `ui_render`. Expect a flat-shaded low-poly terrain you can orbit and zoom; drag **Seed**: the terrain changes within 100 ms; reload: the sliders keep their values and the terrain is regenerated (derived `_heights` is not saved).
3. A logic function that never returns (edit the recipe: `while(true){}`) → the interface shows `step: timed out` inline, the page stays responsive, the state is unchanged.
4. A logic function that calls `fetch('/api/v1/agents')` → it throws (`fetch is not defined`/CSP), inline error.
5. A `Table` with 2000 rows and a `repeat` over 5000 items → refused with the limit named.

### 3b. **[live model]** from a plain prompt, no skill installed

Create an agent on a real provider with **no skills**. Prompt each, once, in a new topic, and record: calls made (`ui_components` first or not), attempts to a valid spec, and whether first draw shows a function error.

- "Build me an interactive chess board I can click to play (both sides)."
- "Make a low-poly 3D terrain generator with sliders for seed and roughness."
- "Show last week's task runs by status with a chart and a table."
- "A tip calculator with a bill amount, a percentage slider and the result."
- "A small mandala drawn on a canvas that rotates slowly."

Pass bars: SC-009 (8/10 first-draw clean for chess and terrain), SC-010 (95% valid within two attempts across the 20-prompt set), SC-001/SC-002 (compare against the old tool on the same dashboard prompt: time to first draw and output tokens).

## 4. Quality gates

```sh
cargo clippy && cargo test                       # ends_turn unit tests
cd webui && npm run typecheck && npm test        # catalogue/recipe/error-message/runner tests
cd webui && npx playwright test generative-ui    # chess + terrain recipes, axe check
node scripts/client-tools-e2e.mjs                # §1
```
