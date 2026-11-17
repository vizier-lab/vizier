# ui_render guide

`ui_render` draws an interactive HTML page in your reply: an app, a game, a simulation, a dashboard, a form. The person sees it inside the chat.

## The page model

- A page is a **complete HTML document** (or a fragment, placed in the body). Write markup, `<style>` and `<script>` inline; `<script type="module">` works.
- A successful call returns `{"status": "rendered", "page": "pg_…", "version": N, "verified": …}`. Keep `page` and `version`: you need them to change the page.
- **To change a page, send edits, not the whole page again:**

  ```python
  ui_render(page="pg_…", base_version=3, edits=[
      {"find": "background: #eee", "replace": "background: #a0703c"},
  ])
  ```

  Edits apply in order, all or nothing. Each `find` must occur **exactly once**: copy it exactly, whitespace included. For a sweeping change send `html=` instead. `title` renames the page.
- `base_version` must be the page's **latest** version. A page can be changed only in the conversation it was drawn in.
- The page's saved state is shared by all its versions: an edit does not reset a game in progress.

## When it fails

The call raises an error whose message is JSON with a `code`: `invalid_input` (`problems` lists them), `too_large` (over 1 MiB), `unknown_page` / `other_conversation`, `stale_base` (use `latest`), `edit_not_found` / `edit_ambiguous` (make that `find` exact and unique).

`problems` means the version is **saved** but an open tab found problems in its first two seconds: uncaught errors, `console.error`, anything the page policy blocked, an invalid `<vz-*>` attribute, or a page that stopped responding. Fix it with edits based on that `version`; the turn goes on.

`verified: false` means no tab checked it (`reason: "no_tab"` or `"timeout"`). It is saved and drawn when the person opens the chat.

## Rules of the page

- **No network from script.** `fetch`, `XMLHttpRequest`, `WebSocket` and remote scripts, styles or fonts are blocked. Images, audio and video may come from `https://` addresses, or use `vizier.files.url(name)` or a `data:` URL.
- **Never block the page.** Drive animation with `requestAnimationFrame`, not `while` loops. Anything longer than about 50 ms goes in `vizier.worker`. A page that blocks can freeze the tab and opens paused next time.
- No popups, no navigation, no access to the WebUI. Limits: 1 MiB per version, 100 edits per call, a 120-character title.

## Look

- **Plain HTML already looks like Vizier**: the font, headings, links, `button` (`type="submit"` or `class="primary"` is the filled one), `input`, `select`, `textarea`, `table`, `code` and `pre` are styled. Kit rules have no specificity, so your styles always win.
- Layout classes: `vz-stack` (column), `vz-row` (wrapping row), `vz-grid` (equal columns; `--vz-min` sets the least width, default `160px`), `vz-panel` (bordered), `vz-muted` (quieter text). `data-gap="4|8|12|16|24"` sets the gap.
- The theme's colours are CSS variables: `--vz-ground`, `--vz-surface`, `--vz-raised`, `--vz-border`, `--vz-text`, `--vz-text-muted`, `--vz-text-faint`, `--vz-ink`, `--vz-live`, `--vz-failed`, `--vz-retrying`, `--vz-chart-1` … `--vz-chart-5`. `<html data-theme="light|dark">` says which theme is on; `vizier:theme` fires on `window` when it changes. Keep pages calm: colour is for meaning.

## Elements

Ready-made pieces drawn with the WebUI's own components. **Call `ui_describe_elements(names=[…])` for the attributes, events and an example of the ones you use** (several at once).

| Element | What |
|---|---|
| `vz-card` | a bordered panel with a title, holding any HTML |
| `vz-stat` | one figure with its label and change |
| `vz-table` | rows of data under columns |
| `vz-chart` | bar, line, area or pie chart |
| `vz-tabs` | tabs over `<section label>` children |
| `vz-field` | a labelled input: text, number, select, checkbox, switch, slider |
| `vz-badge` | a small label |
| `vz-callout` | a note, hint or warning |
| `vz-progress` | a progress bar |
| `vz-image` | an image from the conversation's files or https |

- Attributes are text; a `json` attribute takes JSON in single quotes (`rows='[…]'`) or a property from script (`table.rows = data`).
- An invalid attribute draws a red box and is reported as a problem.
- Events bubble: `el.addEventListener("change", e => e.detail.value)`. Children move into `el.body`; a `vz-field`'s value is `el.value`.

## The bridge: `window.vizier`

| Call | What it does |
|---|---|
| `vizier.page` | `{id, version}` of this page |
| `vizier.state.get()` / `.set(value)` | the page's saved JSON state (`null` at first), kept across reloads and versions |
| `vizier.state.subscribe(fn)` | `fn(value)` when another tab changes it |
| `await vizier.send(text, data?)` | a message to you as the person's next turn |
| `await vizier.files.url(name)` | a file of this conversation as a `data:` URL |
| `vizier.worker(fn)` | heavy work on its own thread |
| `vizier.has(name)` | whether an ability exists |

- **`vizier.send`** works only within 10 seconds of the person's own click or key press, at most once a second (else `no_interaction` / `rate_limited`). Call it from a click handler: that is how a game against you continues.
- **`vizier.worker(fn)`** returns `{run(input, {timeoutMs}), terminate()}`. `fn` is self-contained (gets `input`, returns a value or promise). `run` rejects with `timeout` (default 10 000 ms, at most 60 000). For a longer program use `<script type="vz-worker" id="ai">self.run = (input) => …</script>` and `vizier.worker("#ai")`.

## Libraries

Import by name, with no network: `import * as THREE from "three"` in a `<script type="module">`.

| Name | What |
|---|---|
| `three` | three.js 0.186 (3D) |

## Examples

`ui_examples(kind)` returns a whole page: `dashboard`, `form`, `game` (canvas loop with saved state), `worker` (an opponent thinking on its own thread), `three` (a 3D scene).

```html
<h2>Signups</h2>
<div class="vz-grid"><vz-stat label="New" value="1284" delta="+12%"></vz-stat><vz-stat label="Activated" value="0.61" format="percent"></vz-stat></div>
<vz-card title="By day"><vz-chart kind="bar" data='[{"label":"Mon","value":160},{"label":"Tue","value":210}]'></vz-chart></vz-card>
```
