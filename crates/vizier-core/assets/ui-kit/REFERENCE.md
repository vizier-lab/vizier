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

### Kit CSS

Every page starts with the kit's stylesheet, so **plain HTML already looks like Vizier**: the font, headings, paragraphs, links, `button` (a `type="submit"` or `class="primary"` button is the filled one), `input`, `select`, `textarea`, `table`, `code` and `pre` are styled in the theme. Every kit rule has no specificity, so any style of yours overrides it. The body has 16 px of padding.

Layout classes:

| Class | What |
|---|---|
| `vz-stack` | a column with a gap |
| `vz-row` | a row that wraps, items centred |
| `vz-grid` | equal columns that drop to fewer as the page narrows; set `--vz-min` (default `160px`) for a column's least width |
| `vz-panel` | a bordered panel |
| `vz-muted` | quieter text |

Set the gap with `data-gap="4|8|12|16|24"` on a `vz-stack`, `vz-row` or `vz-grid`.

## Elements

Ready-made pieces, drawn with the WebUI's own components: write the tag and its attributes, and it draws itself.

- Attributes are text. A `json` attribute takes JSON in single quotes (`rows='[…]'`), or set it from script as a property of the same name (`table.rows = data`), which redraws it.
- `*` marks a required attribute. An invalid attribute draws a red box naming the problem, and it is reported like an error, so a check tells you.
- Events are DOM events on the element, and they bubble: `el.addEventListener("change", e => e.detail.value)`.
- An element's children (in a card, callout or tabs) are moved into `el.body`; append to `el.body` to add more later.
- A `vz-field`'s current value is `el.value`.

### `<vz-card>`

A bordered panel with an optional title, holding any HTML.

| Attribute | Type | |
|---|---|---|
| `title` | string | heading of the card |
| `subtitle` | string | one line under the title |
| `tone` | `default` · `error` | error draws it as a danger card |

Children: any HTML, drawn inside the card (add more later through `card.body`).

```html
<vz-card title="This week" subtitle="Signups by day"><p>Steady growth.</p></vz-card>
```

### `<vz-stat>`

One figure with its label, and an optional change under it.

| Attribute | Type | |
|---|---|---|
| `label` * | string | what the figure is |
| `value` * | number | the figure |
| `format` | `number` · `percent` · `currency` · `bytes` · `duration` | how a number is shown: percent takes a fraction (0.42 is 42%), duration takes seconds |
| `delta` | string | a change to show under it, as text ("+12% on last week") |
| `tone` | `default` · `error` | error draws the figure in the failed colour |

```html
<vz-stat label="Activated" value="0.61" format="percent" delta="+4 pts"></vz-stat>
```

### `<vz-table>`

Rows of data under labelled columns, numbers right-aligned on request.

| Attribute | Type | |
|---|---|---|
| `columns` * | json | list of {key, label, align?: "start"|"end", format?} |
| `rows` | json | list of objects keyed by the column keys |
| `label` | string | what the table holds, for screen readers |
| `max-rows` | number | rows shown before "Showing N of M" (default 200) |
| `selectable` | boolean | rows can be pressed; fires select |

- Event `select`: `detail` is {row, index}: the row pressed (selectable only).

```html
<vz-table label="Scores" columns='[{"key":"name","label":"Name"},{"key":"score","label":"Score","align":"end"}]' rows='[{"name":"Ada","score":91},{"name":"Lin","score":84}]'></vz-table>
```

### `<vz-chart>`

A bar, line, area or pie chart in the theme colours.

| Attribute | Type | |
|---|---|---|
| `kind` * | `bar` · `line` · `area` · `pie` | the kind of chart |
| `data` * | json | list of {label, value, series?}; points with the same series form one line or bar group |
| `title` | string | heading over the chart |
| `height` | number | height in px (default 220) |
| `legend` | boolean | show the series legend |

- Event `press`: `detail` is {label, value}: the bar, point or slice pressed.

```html
<vz-chart kind="bar" title="Signups" data='[{"label":"Mon","value":120},{"label":"Tue","value":180}]'></vz-chart>
```

### `<vz-tabs>`

Tabs over sections: each child `<section label="…">` is one tab.

| Attribute | Type | |
|---|---|---|
| `value` | string | the label of the tab shown first (default the first) |
| `label` | string | what the tabs choose between, for screen readers |

Children: `<section label="…">` elements, one per tab.

- Event `change`: `detail` is {value}: the label of the tab chosen.

```html
<vz-tabs><section label="Summary"><p>…</p></section><section label="Details"><p>…</p></section></vz-tabs>
```

### `<vz-field>`

A labelled input: text, multi-line text, number, select, checkbox, switch or slider.

| Attribute | Type | |
|---|---|---|
| `kind` | `text` · `textarea` · `number` · `select` · `checkbox` · `switch` · `slider` | the kind of input (default text) |
| `label` * | string | the label shown with it |
| `name` | string | a name for your own code; it is in every event |
| `value` | string | the starting value ("true" for a checked checkbox or switch) |
| `placeholder` | string | hint inside an empty text or number field |
| `options` | json | select only: list of strings, or of {value, label} |
| `min` | number | number and slider: the least value |
| `max` | number | number and slider: the greatest value |
| `step` | number | number and slider: the step |
| `help` | string | a line of help under it |
| `disabled` | boolean | cannot be changed |

- Event `input`: `detail` is {name, value}: on every change while typing or dragging.
- Event `change`: `detail` is {name, value}: when a value is committed (and on every change for select, checkbox and switch).

```html
<vz-field kind="select" name="range" label="Range" value="7d" options='["24h","7d","30d"]'></vz-field>
```

### `<vz-badge>`

A small label, its text the element's own text.

| Attribute | Type | |
|---|---|---|
| `tone` | `default` · `accent` · `ok` · `warn` · `error` | colour of the element |

Children: the text of the badge.

```html
<vz-badge tone="ok">live</vz-badge>
```

### `<vz-callout>`

A note set apart from the page: a quiet hint, or a warning or error.

| Attribute | Type | |
|---|---|---|
| `tone` | `default` · `accent` · `ok` · `warn` · `error` | colour of the element |
| `title` | string | heading of the note |

Children: the text of the note (any HTML).

```html
<vz-callout tone="warn" title="Estimates">These numbers are from a sample.</vz-callout>
```

### `<vz-progress>`

A progress bar with its label.

| Attribute | Type | |
|---|---|---|
| `value` * | number | progress from 0 to max |
| `max` | number | the value that is complete (default 1) |
| `label` | string | what is progressing |
| `tone` | `default` · `error` | error draws it as failed |

```html
<vz-progress label="Import" value="0.4"></vz-progress>
```

### `<vz-image>`

An image from the conversation's files or an https address.

| Attribute | Type | |
|---|---|---|
| `file` | string | the name of a file in this conversation |
| `src` | string | an https: or data: address (when there is no file) |
| `alt` * | string | what the image shows |
| `width` | number | width in px |
| `height` | number | height in px |

- Event `load`: `detail` is {}: the image loaded.
- Event `error`: `detail` is {message}: it could not be loaded.

```html
<vz-image file="forecast.png" alt="Forecast for the week"></vz-image>
```

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

### A dashboard

```html
<h2>Signups · week 46</h2>
<div class="vz-grid" style="--vz-min:140px">
  <vz-stat label="New" value="1284" delta="+12% on last week"></vz-stat>
  <vz-stat label="Activated" value="0.61" format="percent"></vz-stat>
  <vz-stat label="Churned" value="0.032" format="percent" tone="error"></vz-stat>
</div>
<vz-card title="By day">
  <vz-chart kind="bar" data='[{"label":"Mon","value":160},{"label":"Tue","value":210},{"label":"Wed","value":190},{"label":"Thu","value":260}]'></vz-chart>
</vz-card>
<vz-table id="top" label="Top sources" columns='[{"key":"source","label":"Source"},{"key":"n","label":"Signups","align":"end"}]'></vz-table>
<script>
  document.getElementById('top').rows = [{ source: 'Search', n: 512 }, { source: 'Referral', n: 301 }]
</script>
```

### A form that answers you

```html
<vz-card title="Plan the trip">
  <div class="vz-stack">
    <vz-field id="city" name="city" label="City" placeholder="Lisbon"></vz-field>
    <vz-field id="days" kind="slider" name="days" label="Days" value="4" min="1" max="14"></vz-field>
    <vz-field id="pace" kind="select" name="pace" label="Pace" value="easy" options='["easy","busy"]'></vz-field>
    <div class="vz-row"><button type="submit" id="go">Plan it</button></div>
  </div>
</vz-card>
<script>
  const $ = (id) => document.getElementById(id)
  $('go').onclick = () => vizier.send('Plan my trip', { city: $('city').value, days: $('days').value, pace: $('pace').value })
</script>
```

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
