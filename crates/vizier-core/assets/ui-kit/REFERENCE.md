# ui_render guide

`ui_render` draws an interactive HTML page in your reply. Anything a web page can be: an app, a game, a simulation, a dashboard, a form. The person sees it inside the chat, with a button to open it full screen in the chat panel.

## The page model

- A page is a **complete HTML document** (or a fragment; it is placed in the body). Write markup, `<style>` and `<script>` inline. `<script type="module">` works.
- Every page you draw is **kept with its versions**. A successful call returns `{"status": "rendered", "page": "pg_…", "version": N, "verified": …}`. Keep the `page` id and `version`: you need them to change the page.
- **To change a page, send edits, not the whole page again.** This is much faster:

  ```python
  ui_render(page="pg_…", base_version=3, edits=[
      {"find": "background: #eee", "replace": "background: #a0703c"},
      {"find": "</aside>", "replace": "<ol id=\"moves\"></ol></aside>"},
  ])
  ```

  - Edits apply in order, each to the text the edit before produced.
  - Each `find` must occur **exactly once** at that point. Copy it exactly, whitespace included, and make it long enough to be unique.
  - The whole set applies or nothing does.
- For a sweeping change, send a full replacement: `ui_render(page="pg_…", base_version=3, html="…")`. `title` is optional, to rename the page.
- `base_version` must be the page's **latest** version. If it is not, the call is refused with the latest number.
- You can change a page only in the conversation you drew it in.
- The page's **saved state** (see the bridge) is shared by all its versions: an edit does not reset a game in progress.

## When it fails

The call raises an error whose message is JSON with a `code`:

| code | meaning | what to do |
|---|---|---|
| `invalid_input` | the arguments are wrong; `problems` lists them all | fix the arguments |
| `too_large` | the version is over `limit` bytes (1 MiB) | make it smaller |
| `unknown_page` / `other_conversation` | no such page here | draw a new page |
| `stale_base` | the page has moved on; `latest` is its newest version | base your edits on `latest` |
| `edit_not_found` / `edit_ambiguous` | edit `edit` matched 0 or `count` times | make that `find` exact and unique |
| `problems` | **saved**, but an open tab found problems; see `page`, `version`, `problems` | fix it with edits based on that `version` |

A `problems` error lists what the page raised in its first two seconds: uncaught errors and rejections, `console.error` calls, anything the page policy blocked, and a page that stopped responding. Each has a `message` and, when known, a `line` and `column`. The turn goes on so you can fix it.

`verified: false` means no tab was open to check it (`reason: "no_tab"`), or the tab did not answer in time. The page is saved and drawn when the person opens the chat.

## Rules of the page

- **No network from script.** `fetch`, `XMLHttpRequest`, `WebSocket` and scripts, styles or fonts from other sites are blocked.
- **Images, audio and video may come from any `https://` address.** `http://` is blocked.
- For an image that must always show, use a file of this conversation through `vizier.files.url(name)` (see the bridge), or a `data:` URL.
- **Never block the page.**
  - Drive animation and game loops with `requestAnimationFrame`, not `while` loops.
  - Put anything that can run longer than about 50 ms (a chess search, path-finding, a physics step run many times) in `vizier.worker`, which runs on its own thread.
  - A page that blocks may freeze the person's whole tab in some browsers. Next time it opens paused.
- The page cannot open popups, navigate the chat, or read anything of the WebUI.
- Limits: 1 MiB per version, 100 edits per call, a 120-character title.

## Look

- The page follows the person's theme. The theme's colours are CSS variables on `:root`. Use them, so the page looks right in light and dark:
  - `--vz-ground` (page background)
  - `--vz-surface` and `--vz-raised` (panels)
  - `--vz-border`
  - `--vz-text`, `--vz-text-muted` and `--vz-text-faint`
  - `--vz-ink` (strong fills)
  - `--vz-live`, `--vz-failed` and `--vz-retrying` (states)
  - `--vz-chart-1` … `--vz-chart-5` (data)
- `<html data-theme="light|dark">` says which theme is on, and a `vizier:theme` event fires on `window` when it changes.
- Your own styles always win. Keep pages calm: the WebUI is mostly monochrome, and colour is for meaning.

## The bridge: `window.vizier`

`window.vizier` exists before your scripts run.

| Call | What it does |
|---|---|
| `vizier.version` | `1` |
| `vizier.has("state")` | whether an ability exists; check before using one |
| `vizier.page` | `{id, version}` of this page |
| `vizier.state.get()` | the page's saved state (`null` at first) |
| `vizier.state.set(value)` | save the state (any JSON); saved at most twice a second, kept across reloads and versions |
| `vizier.state.subscribe(fn)` | `fn(value)` when another tab changes it; returns an unsubscribe function |
| `await vizier.send(text, data?)` | send a message to you as the person's next turn, with optional JSON `data` |
| `await vizier.files.url(name)` | a file of this conversation, by name, as a `data:` URL |
| `vizier.worker(fn)` | heavy work on its own thread; see below |

- **`vizier.send`** works only within 10 seconds of the person clicking or typing in the page, and at most once a second; otherwise it rejects with `no_interaction` or `rate_limited`. Call it from a click handler. This is how a game against you continues: the page sends the person's move, and you answer in your next turn (often with an edit, or by updating the state).
- **`vizier.worker(fn)`** returns `{run(input, {timeoutMs}), terminate()}`.
  - `fn` must be self-contained: it gets `input` and returns a value or a promise, with no access to the page's variables.
  - `run` resolves with the result, or rejects with `timeout` after `timeoutMs` (default 10 000, at most 60 000).
  - For a longer program, put it in `<script type="vz-worker" id="ai">…</script>`, have it define `self.run = (input) => …`, and call `vizier.worker("#ai")`.

## Libraries

Import these by name, with no network:

```html
<script type="module">
  import * as THREE from "three"
</script>
```

| Name | What |
|---|---|
| `three` | three.js 0.186 (3D) |

## Examples

### A canvas game loop

```html
<canvas id="c" width="480" height="320" style="width:100%;background:var(--vz-surface);border-radius:8px"></canvas>
<script>
  const c = document.getElementById('c'), g = c.getContext('2d')
  const s = vizier.state.get() ?? { x: 40, y: 40, score: 0 }
  const keys = new Set()
  addEventListener('keydown', e => keys.add(e.key))
  addEventListener('keyup', e => keys.delete(e.key))
  function frame() {
    if (keys.has('ArrowRight')) s.x += 3
    if (keys.has('ArrowLeft')) s.x -= 3
    g.clearRect(0, 0, c.width, c.height)
    g.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--vz-ink')
    g.fillRect(s.x, s.y, 16, 16)
    requestAnimationFrame(frame)
  }
  setInterval(() => vizier.state.set(s), 1000)
  requestAnimationFrame(frame)
</script>
```

### An opponent that thinks in a worker

```html
<button id="go">My move: e2e4</button><p id="out"></p>
<script type="vz-worker" id="ai">
  self.run = ({ position }) => {
    let best = 0
    for (let i = 0; i < 5e7; i++) best = (best + i) % 97   // stands in for a real search
    return { move: "e7e5", score: best }
  }
</script>
<script>
  const ai = vizier.worker('#ai')
  document.getElementById('go').onclick = async () => {
    document.getElementById('out').textContent = 'Thinking…'
    const { move } = await ai.run({ position: 'start' }, { timeoutMs: 20000 })
    document.getElementById('out').textContent = 'Reply: ' + move
    await vizier.send('I played e2e4; the board replied ' + move, { move: 'e2e4' })
  }
</script>
```
