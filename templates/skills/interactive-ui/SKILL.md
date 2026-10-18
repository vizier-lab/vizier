---
name: interactive-ui
author: vizier
description: Use before calling ui_render_interactive_ui (or any time you draw an HTML page, guide, infographic, dashboard, chart, table or form in the WebUI) so the page looks designed, reads well in both themes and fits the reply.
keywords:
- ui
- html
- page
- infographic
- guide
- dashboard
- chart
- table
- form
- interactive
- render_interactive_ui
version: 3
---

# Interactive pages in the WebUI

`ui_render_interactive_ui(title=…, html=…)` draws a page inside your reply. It is in the `ui`
category and only exists while the person is on the WebUI. This skill is how to make that page
look designed, like a printed piece, rather than a bare web page or a generic admin panel.

## 1. Pick a register first

Decide what the page *is* before writing any HTML. The register decides the typography, the
spacing and how much the built-in kit you use.

| Register | For | Looks like | Kit |
|---|---|---|---|
| **Editorial** | explainers, guides, histories, infographics, reports, rules of a game | a printed booklet or field guide: big serif title, small tracked labels, hairline rules, prose | baseline only; write your own type (section 3) |
| **Dashboard** | status, metrics, comparisons, results of a query | a compact instrument panel: stats, tables, small charts | use it (section 2) |
| **App** | games, calculators, forms, pickers | a single tool: the board or control first, little chrome | use it for controls, your own CSS for the board |

Then commit to a *form* within the register and let it name your sections: a rulebook has
chapters and plates, a field guide has numbered sections and figures, a dossier has a
cover sheet and exhibits. A page that knows what it is imitating looks deliberate.

## 2. The theme and the kit

Every page is drawn with a stylesheet that matches the WebUI's light or dark theme:

- **Styled elements**: `h1`–`h4`, `p`, `a`, `small`, `ul`/`ol`, `table`/`th`/`td`, `code`, `pre`,
  `kbd`, `blockquote`, `hr`, `button`, `input`, `select`, `textarea`, `label`, `fieldset`,
  `details`/`summary`, `progress`, `meter`.
- **Layout classes**: `.card`, `.grid` (responsive columns, 180px minimum), `.row`, `.stack`,
  `.scroll` (horizontal scroll, for wide tables).
- **Content classes**: `.stat` (`<b>value</b><span>label</span>`), `.badge` with `.accent`,
  `.ok`, `.warn` or `.err`, `.bar` (track, with an inner `<span style="width:40%">`),
  `button.primary`, `.muted`, `.faint`, `.num` (tabular figures, right-aligned).
- **Variables**: `--text --muted --faint` (ink), `--surface --border --tint` (surfaces and
  subtle fills), `--accent --accent-soft --accent-text`, status `--ok --warn --error` with
  `--ok-bg --warn-bg --error-bg`, charts `--series-1` … `--series-6`, and `--radius --shadow
  --font --mono`.

The kit has zero specificity, so any rule you write wins. For a dashboard, it is most of what
you need:

```html
<h3>Release status</h3>
<div class="grid">
  <div class="card stat"><b>12</b><span>Merged this week</span></div>
  <div class="card stat"><b>3</b><span>Waiting on review</span></div>
</div>
<table>
  <thead><tr><th>PR</th><th>Owner</th><th>State</th></tr></thead>
  <tbody><tr><td>#212 Usage export</td><td>ana</td><td><span class="badge ok">merged</span></td></tr></tbody>
</table>
```

**Colour: alias, never hard-code.** When you want your own names, map them onto the theme's
variables at the top of your CSS, then use only your names:

```css
:root{
  --ink:var(--text); --soft:var(--muted); --dim:var(--faint);
  --rule:var(--border); --mark:var(--accent);
  --mark-soft:color-mix(in srgb, var(--accent) 12%, transparent);
}
```

A hex colour is right in exactly one place: inside a chart series you cannot express with
`--series-1…6`, and even then check it on both themes. Hard-coded palettes (a dark `--bg`, a
gradient glow behind `body`) look striking in one theme and broken in the other.

## 3. Editorial typography

Most of what makes a guide or infographic look designed is type, not boxes. Use two voices:

- **Display serif** for the title, section titles, big numbers and italic subtitles.
- **Tiny tracked capitals** (sans, 9.5–11px, `letter-spacing:.14em–.3em`, uppercase) for
  eyebrows, section numbers, table headers and figure labels.

The contrast between the two does the work. Open each level with the same sequence:
**eyebrow → title → italic subtitle → (optional) small ornament**.

A starting point; rename and extend per page (prefix your classes, see section 6):

```css
:root{
  --ink:var(--text); --soft:var(--muted); --dim:var(--faint); --rule:var(--border);
  --mark:var(--accent); --mark-soft:color-mix(in srgb, var(--accent) 12%, transparent);
  --serif:"Iowan Old Style","Palatino Linotype",Palatino,Georgia,serif;
  --sans:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
}
body{font-family:var(--serif);font-size:16.5px;line-height:1.62;color:var(--ink)}
.ed-eyebrow,.ed-num,.ed-fig b,.ed-page th{
  font-family:var(--sans);font-size:10.5px;font-weight:600;
  letter-spacing:.22em;text-transform:uppercase;color:var(--mark)}
.ed-title{font-weight:400;font-size:clamp(36px,8vw,64px);line-height:1;
  letter-spacing:-.02em;margin:.25em 0 .2em}
.ed-title em{color:var(--mark)}
.ed-sub{font-style:italic;color:var(--soft);max-width:46ch;margin:0 0 1.2em}
.ed-sec{border-top:1px solid var(--rule);padding-top:14px;margin-top:clamp(32px,6vw,52px)}
.ed-sec h2{font-weight:400;font-size:clamp(22px,4.5vw,30px);line-height:1.2;margin:.3em 0 .2em}
.ed-fig{margin:24px 0;padding:14px 0;border-top:1px solid var(--ink);border-bottom:1px solid var(--rule)}
.ed-fig figcaption{font-family:var(--sans);font-size:11px;line-height:1.5;color:var(--soft);margin-top:10px}
.ed-fig b{display:block;margin-bottom:3px}
.ed-stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:1px;
  background:var(--rule);border:1px solid var(--rule)}
.ed-stats>div{background:var(--surface);padding:14px 16px}
.ed-stats b{display:block;font-weight:400;font-size:28px;line-height:1.1;letter-spacing:-.02em}
.ed-stats span{display:block;margin-top:5px;font-family:var(--sans);font-size:10px;
  letter-spacing:.1em;text-transform:uppercase;color:var(--soft)}
.ed-note{margin:20px 0;padding:12px 16px;border-left:2px solid var(--mark);
  background:var(--mark-soft);font-style:italic;color:var(--soft)}
.ed-page table{border-collapse:collapse;width:100%}
.ed-page th{color:var(--soft);font-size:10px;text-align:left;border-bottom:1px solid var(--ink);padding:0 10px 8px 0}
.ed-page td{border-bottom:1px solid var(--rule);padding:7px 10px 7px 0;vertical-align:top}
```

```html
<div class="ed-page">
  <div class="ed-eyebrow">A field guide · ten eras</div>
  <h1 class="ed-title">How we <em>govern</em> ourselves</h1>
  <p class="ed-sub">From bands with no rulers to the networked state, in ten steps.</p>
  <div class="ed-stats">
    <div><b>200,000</b><span>Years before the first state</span></div>
    <div><b>193</b><span>UN member states today</span></div>
  </div>
  <section class="ed-sec">
    <div class="ed-num">01 · The timeline</div>
    <h2>Ten overlapping eras</h2>
    <p>…</p>
    <figure class="ed-fig">…chart…<figcaption><b>Fig 1 — Largest polity by era</b>Population, log scale.</figcaption></figure>
  </section>
</div>
```

Rules of the register:

- **Rules, not boxes.** Separate with 1px hairlines: an ink line above a figure, a faint rule
  below it and between table rows, `gap:1px` over the rule colour for a grid of stats. Avoid
  rounded cards, shadows and filled panels; at most one tinted note (`.ed-note`) per section.
- **One accent, used sparingly**: eyebrows, section numbers, the active state, one emphasised
  word in the title. Everything else is `--ink`, `--soft` or `--dim`.
- **Prose is welcome.** A lede under the title and a few real paragraphs per section are what
  make it read like a guide. Keep lines under ~70 characters (`max-width:66ch` on paragraphs).
- **Room to breathe.** Sections 32–52px apart (`clamp`), the display title as large as the
  bubble allows. Large type is fine here; tiny type is only for labels.
- **Number things.** `01`, `Chapter one`, `Fig 2`, `Plate III`: numbering gives a long page a
  structure the reader can hold.
- **Small details**: `font-variant-numeric:tabular-nums` on figures, an italic subtitle, a
  drop cap on a chapter's first paragraph, a `◇` or thin rule as an ornament. One or two per
  page, not all of them.

## 4. Design for where it is drawn

- **It sits in a chat bubble**, often 500–700px wide, sometimes a phone, and can be expanded
  to the chat's width. No fixed widths and no 100vh layouts; use `clamp()`, `.grid`,
  `auto-fit` or `flex-wrap` so things reflow, and wrap wide tables in `.scroll`.
- **Leave the background transparent.** Never set `body` or `html` backgrounds; the reply
  already sits on the WebUI background. Use `var(--surface)` for any filled area.
- **Both themes.** Every colour is a variable or an alias of one. Check that text on any fill
  you made still reads in light and dark.
- **Dashboards and apps stay compact**: lead with the answer (a stat, the board), keep prose to
  the reply text. Editorial pages may be long; give them a contents list or numbered sections.

## 5. Charts

There is no network, so no chart libraries: draw with inline SVG (or HTML bars with `.bar`).

- Series colours are `var(--series-1)` … `var(--series-6)`, **in that order**, assigned to
  each entity once so a filter never repaints them. Six is the limit; fold the rest into
  "Other". On an editorial page, a single series can simply be `var(--mark)`, with the
  highlighted value at full opacity and the rest faded (`opacity:.3`).
- SVG text is black by default and disappears in dark mode: put `fill="currentColor"` on the
  `<svg>` or the `<text>`, and use `var(--muted)` for axis labels.
- Grid lines `stroke="var(--border)"`, thin marks (2px lines), no dual axes.
- For two or more series, show a legend and label the marks directly where there is room. A
  value should never be readable only through its colour.
- Use `viewBox` with `width="100%"` so the chart scales with the bubble. Position the points
  of a dense strip or timeline (50+ items) absolutely in percentages instead of with a
  `repeat(N, 1fr)` grid, which won't shrink below its content and overflows.
- Hovering a mark should show its value (a `<title>` inside the SVG element is enough).
- Put a chart in a figure with a caption label (`Fig 1 — …`) on editorial pages.

## 6. Making it interactive

The page's script gets `vizier`:

- `vizier.send(text, data?)` posts `text` as the person's message and starts a turn. Use it for
  a real choice the person makes (pick an option, submit a form). It only works after they
  interacted with the page, at most once a second. Make `text` read naturally in the chat ("Pick
  plan B"), and put the machine-readable part in `data`.
- `vizier.saveState(value)` / `vizier.state` / `vizier.onState(cb)` remember UI state (a
  selected tab, a checked list) without a turn. On load, redraw from `vizier.state` if it is not
  `null`, so a page reopened later looks the way the person left it.
- Use real `<button>`s and `<label>`s around inputs, so the page works with a keyboard and a
  screen reader. Style selected states with `aria-pressed="true"`, in the accent.

## 7. Building big pages

- The code-mode sandbox refuses one long string literal: a multi-line `"""…"""` of a couple of
  KB (about 150 lines) can already fail with `SyntaxError: Source is too deeply nested`. Build a
  large page from chunks of about 2 KB instead of one huge literal.
- Every `execute_python` run is stateless: variables are gone when it ends. Keep the page in the
  scratchpad, which lasts the whole turn. Send each chunk exactly once, in its own script:
  `append_tmp(name="page.html", content=style)` (use `write_tmp` for the first chunk), then
  `append_tmp(name="page.html", content=body)`, and so on. When it is complete, render it in one
  script: `html = read_tmp(name="page.html")["content"]` then
  `ui_render_interactive_ui(title=…, html=html)`.
- Never re-send a chunk you already wrote, and don't print `len(...)` or the HTML to "check"
  it: `append_tmp` returns the file's size, and a long print only costs tokens. If a chunk is
  wrong, `write_tmp` the file again and resend what changed.
- Prefix your own class names per page or component (`.ed-title`, `.timeline-tick`, not
  `.title`, `.num` or `.tick`). Short generic names collide with the kit (`.num`, `.row`,
  `.bar`, `.stat` are taken) or with each other.
- Keep the page under 256 KB. Images only as `data:` URLs, and only small ones.
- Call the tool once per reply, with the final page. A second call replaces the first.

## 8. Check before you send

- If the page has logic (state, calculations, a game), run that logic first: parse the script
  (`new Function(code)` under node, if you have a shell with node) and drive its functions with
  a stubbed `document`/`vizier`.
- If you have a shell with a headless Chromium-based browser, render the page and measure it:
  `--dump-dom` with an appended script that reports `document.documentElement.scrollWidth`
  against `clientWidth`, and any element whose `getBoundingClientRect().right` exceeds the
  width, at about 500px and 1000px. Test 500px, not 375px: headless window sizes clamp.
- Otherwise re-read the HTML once for fixed widths, hex colours outside the alias block,
  `body` backgrounds and missing `fill="currentColor"`; those cause most broken pages.

## 9. When it says `deferred`

The tool returns `{"status": "deferred"}` when no WebUI tab is open. The page appears the next
time the person opens the conversation, so tell them that instead of saying it is on screen.
