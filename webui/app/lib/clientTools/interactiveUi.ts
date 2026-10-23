// What the agent reads about `render_interactive_ui`. Kept apart from `builtin.tsx` (JSX) so
// its size can be tested: the server refuses a declaration whose description is over 2048
// bytes (`client::declare`), and with it every request the WebUI sends.
//
// The style lines describe the kit in `pageDocument.ts`; keep the two in step.

export const RENDER_INTERACTIVE_UI_DESCRIPTION = [
  'Show an interactive HTML page (a chart, a table, buttons, a form) above your reply text in the WebUI; it stays in the conversation.',
  'Write one complete, self-contained page: inline <style> and <script>, images only as data: URLs.',
  'The page has no network access and cannot see the WebUI or the person\'s login.',
  'Its script gets a `vizier` object:',
  '- vizier.send(text, data?) — posts `text` as the person\'s message and starts a turn; `data` (any JSON) reaches you unseen. Only after the person interacts, at most once a second;',
  '- vizier.state — the state the page last saved (null at first), so it can redraw as it was;',
  '- vizier.saveState(value) — save state (any JSON, up to 64 KB); never starts a turn, not shown to you;',
  '- vizier.onState(cb) — called when the page\'s state is saved from another tab.',
  'Style: pages are already themed like the WebUI (light or dark): write plain HTML, add CSS only for what the kit lacks. The `interactive-ui` skill has the full guide.',
  '- Styled with no CSS: headings, p, lists, table, code/pre, blockquote, hr, button, input, select, textarea, label, fieldset, details, progress.',
  '- Classes: .card (surface box), .grid (responsive columns), .row (wrapping flex row), .stack (column), .stat (<b>value</b><span>label</span>), .badge with .accent/.ok/.warn/.err, .bar (track; inner <span style="width:40%">), button.primary, .muted, .faint, .num (tabular, right-aligned), .scroll (wide tables).',
  '- Variables: --text --muted --faint --surface --border --accent --tint --radius; --ok --warn --error (+ -bg); charts --series-1…6 in order. Keep the background transparent; SVG text fill="currentColor".',
  '- Example: <div class="grid"><div class="card stat"><b>42</b><span>Open tasks</span></div><div class="card stat"><b>7</b><span>Due today</span></div></div>',
  'The page appears once your reply is complete. A reply shows one page: calling this again replaces it, so draw the final version once.',
  'Limit 256 KB. Returns {"ui_id", "status": "rendered"}.',
].join('\n')
