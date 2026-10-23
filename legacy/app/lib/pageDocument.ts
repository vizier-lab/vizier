// The documents an interactive page is drawn from (`srcdoc`) and downloaded as. Pure and
// free of JSX, so they can be unit-tested (`node --test app/lib/pageDocument.test.ts`).

export const CSP =
  "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; media-src data:"


// JSON that can sit inside a <script> element.
export const scriptJson = (value: unknown) =>
  JSON.stringify(value ?? null).replace(/</g, '\\u003c')

export function bridge(initialState: unknown): string {
  return `(function () {
  var post = function (message) { parent.postMessage(message, '*') };
  var listeners = [];
  window.vizier = {
    state: ${scriptJson(initialState)},
    send: function (text, data) { post({ type: 'vizier:send', text: String(text), data: data }) },
    saveState: function (value) { window.vizier.state = value; post({ type: 'vizier:state', value: value }) },
    onState: function (cb) { if (typeof cb === 'function') listeners.push(cb) },
  };
  window.addEventListener('message', function (event) {
    if (event.source !== parent || !event.data || event.data.type !== 'vizier:state') return;
    window.vizier.state = event.data.value;
    listeners.forEach(function (cb) { try { cb(event.data.value) } catch (err) { console.error(err) } });
  });
  var last = 0;
  var report = function () {
    var height = Math.ceil(document.documentElement.getBoundingClientRect().height);
    if (height !== last) { last = height; post({ type: 'vizier:height', height: height }) }
  };
  new ResizeObserver(report).observe(document.documentElement);
  window.addEventListener('load', report);
})();`
}

/** The WebUI theme colours a page gets as CSS variables (`--bg`, `--surface`, …). */
export const PALETTE_KEYS = [
  'bg', 'surface', 'border', 'text', 'muted', 'faint', 'accent',
  'ok', 'ok-bg', 'warn', 'warn-bg', 'error', 'error-bg',
] as const
export type Palette = Record<(typeof PALETTE_KEYS)[number], string>

/** The WebUI's own variable each palette colour is read from. */
export const PALETTE_SOURCES: Palette = {
  bg: '--background',
  surface: '--surface',
  border: '--border',
  text: '--text-primary',
  muted: '--text-secondary',
  faint: '--text-tertiary',
  accent: '--accent-primary',
  ok: '--color-success-text',
  'ok-bg': '--color-success-bg',
  warn: '--color-warning-text',
  'warn-bg': '--color-warning-bg',
  error: '--color-error',
  'error-bg': '--color-error-bg',
}

// The theme's values, should a WebUI variable be missing or unusable.
const FALLBACK: Record<'dark' | 'light', Palette> = {
  dark: {
    bg: '#0a0a0b', surface: '#18181b', border: '#27272a', text: '#fafafa', muted: '#a1a1aa', faint: '#71717a',
    accent: '#10B981', ok: '#6ee7b7', 'ok-bg': '#10291f', warn: '#fde68a', 'warn-bg': '#2a2212',
    error: '#f87171', 'error-bg': '#2a1414',
  },
  light: {
    bg: '#f5f4f0', surface: '#f8f8f5', border: '#dcdad4', text: '#1a1a1a', muted: '#4a4a48', faint: '#8a8a88',
    accent: '#10B981', ok: '#065f46', 'ok-bg': '#e3f4ec', warn: '#5c4510', 'warn-bg': '#fdf6e7',
    error: '#b42318', 'error-bg': '#fdecea',
  },
}

/**
 * Chart series colours, in the order to use them: the WebUI's usage-chart hues, stepped so
 * one set passes the categorical checks (lightness band, chroma, colour-blind separation)
 * on both themes' backgrounds. Amber sits under 3:1 on the light background, so a chart
 * using it labels its marks.
 */
export const SERIES = ['#059669', '#7c3aed', '#d97706', '#0284c7', '#db2777', '#0891b2'] as const

/**
 * The kit every page gets, after the variables. Every selector is wrapped in `:where()`, so
 * it weighs nothing and any rule of the page's own wins, whatever its order or specificity.
 * Kept in step with the `interactive-ui` skill and `render_interactive_ui`'s description.
 */
const KIT = [
  '*,*::before,*::after{box-sizing:border-box}',
  'html,body{margin:0;background:transparent}',
  ':where(h1,h2,h3,h4){margin:0 0 .5em;line-height:1.25;font-weight:600}',
  ':where(h1){font-size:1.5em}:where(h2){font-size:1.25em}:where(h3){font-size:1.05em}:where(h4){font-size:1em;color:var(--muted)}',
  ':where(p,ul,ol,dl,blockquote,pre,table,figure,fieldset,details){margin:0 0 .75em}',
  ':where(ul,ol){padding-left:1.25em}:where(li+li){margin-top:.2em}',
  ':where(a){color:var(--accent);text-decoration:none}:where(a:hover){text-decoration:underline}',
  ':where(small){font-size:.85em;color:var(--muted)}',
  ':where(hr){border:0;border-top:1px solid var(--border);margin:1em 0}',
  ':where(code,kbd,pre,samp){font-family:var(--mono);font-size:.9em}',
  ':where(code){background:var(--tint);border-radius:4px;padding:.1em .35em}',
  ':where(pre){background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:10px 12px;overflow:auto}',
  ':where(pre code){background:none;padding:0}',
  ':where(kbd){border:1px solid var(--border);border-bottom-width:2px;border-radius:4px;padding:0 .35em}',
  ':where(blockquote){margin-left:0;padding-left:12px;border-left:3px solid var(--border);color:var(--muted)}',
  ':where(img,video,canvas,svg){max-width:100%}:where(img,video){height:auto}',
  ':where(table){width:100%;border-collapse:collapse;font-size:.93em}',
  ':where(th,td){padding:6px 10px;text-align:left;vertical-align:top;border-bottom:1px solid var(--border)}',
  ':where(th){font-weight:600;font-size:.86em;color:var(--muted)}:where(thead th){border-bottom-width:2px}',
  ':where(tbody tr:hover){background:var(--tint)}',
  ':where(button,input,select,textarea){font:inherit;color:inherit;background:var(--surface);border:1px solid var(--border);border-radius:6px;padding:5px 10px}',
  ':where(button){cursor:pointer;transition:border-color .12s,background-color .12s}',
  ':where(button:hover){border-color:var(--accent)}:where(button:disabled){opacity:.5;cursor:default}',
  ':where(button.primary){background:var(--accent);border-color:var(--accent);color:#fff;font-weight:500}',
  ':where(button.primary:hover){filter:brightness(1.08)}',
  ':where(button:focus-visible,input:focus,select:focus,textarea:focus){outline:2px solid var(--focus);outline-offset:1px;border-color:var(--accent)}',
  ':where(input[type=checkbox],input[type=radio],input[type=range],progress,meter){accent-color:var(--accent);padding:0}',
  ':where(input[type=range]){background:none;border:0}',
  ':where(progress,meter){width:100%;height:8px}',
  ':where(label){display:inline-flex;align-items:center;gap:6px}',
  ':where(fieldset){border:1px solid var(--border);border-radius:var(--radius);padding:10px 12px}:where(legend){padding:0 4px;color:var(--muted)}',
  ':where(details){border:1px solid var(--border);border-radius:var(--radius);padding:8px 12px}',
  ':where(summary){cursor:pointer;font-weight:600}:where(details[open]>summary){margin-bottom:6px}',
  ':where(.card){background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:14px 16px}',
  ':where(.grid){display:grid;gap:12px;grid-template-columns:repeat(auto-fit,minmax(min(100%,180px),1fr))}',
  ':where(.row){display:flex;flex-wrap:wrap;align-items:center;gap:8px}',
  ':where(.stack){display:flex;flex-direction:column;gap:8px}',
  ':where(.stat){display:flex;flex-direction:column;gap:2px}',
  ':where(.stat)>:where(b,strong){font-size:1.6em;font-weight:600;line-height:1.2;font-variant-numeric:tabular-nums}',
  ':where(.stat)>:where(span,small){font-size:.85em;color:var(--muted)}',
  ':where(.badge){display:inline-flex;align-items:center;gap:4px;padding:1px 8px;border-radius:999px;font-size:.8em;font-weight:500;line-height:1.6;background:var(--tint);color:var(--muted)}',
  ':where(.badge.accent){background:var(--accent-soft);color:var(--accent-text)}',
  ':where(.badge.ok){background:var(--ok-bg);color:var(--ok)}',
  ':where(.badge.warn){background:var(--warn-bg);color:var(--warn)}',
  ':where(.badge.err){background:var(--error-bg);color:var(--error)}',
  ':where(.bar){height:8px;border-radius:999px;background:var(--tint);overflow:hidden}',
  ':where(.bar)>:where(span){display:block;height:100%;border-radius:inherit;background:var(--accent)}',
  ':where(.muted){color:var(--muted)}:where(.faint){color:var(--faint)}',
  ':where(.num){font-variant-numeric:tabular-nums;text-align:right}',
  ':where(.scroll){overflow-x:auto}',
  ':where(.card,.stack,.stat,details,blockquote,fieldset,li,td,th)>:where(:last-child){margin-bottom:0}',
].join('')

/** How the page looks where it is drawn: the theme, and its colours. */
export interface PageLook {
  dark: boolean
  palette: Partial<Palette>
}

/** A colour that can sit inside a <style> element, or `null`. */
const safeColour = (colour: string | undefined) =>
  colour && /^[#(),.%\w\s-]+$/.test(colour) && colour.trim() ? colour.trim() : null

/**
 * Base styles: the theme's colours as variables, then the kit, so a page of plain HTML
 * (headings, tables, forms, cards) reads as part of the reply. A page's own styles win.
 */
export function baseStyle(look: PageLook): string {
  const fallback = FALLBACK[look.dark ? 'dark' : 'light']
  const vars = PALETTE_KEYS.map((key) => `--${key}:${safeColour(look.palette[key]) ?? fallback[key]}`).join(';')
  const series = SERIES.map((colour, i) => `--series-${i + 1}:${colour}`).join(';')
  const shadow = look.dark ? '0 1px 3px rgba(0,0,0,.4)' : '0 1px 3px rgba(0,0,0,.08)'
  const derived =
    '--tint:color-mix(in srgb,var(--text) 7%,transparent);' +
    '--accent-soft:color-mix(in srgb,var(--accent) 16%,transparent);' +
    '--accent-text:color-mix(in srgb,var(--accent) 70%,var(--text));' +
    '--focus:color-mix(in srgb,var(--accent) 45%,transparent);' +
    `--shadow:${shadow};--radius:8px;` +
    '--font:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;--mono:ui-monospace,Menlo,Monaco,"Courier New",monospace'
  return (
    `:root{color-scheme:${look.dark ? 'dark' : 'light'};${vars};${series};${derived};` +
    'color:var(--text);font-family:var(--font);font-size:14px;line-height:1.5}' +
    KIT
  )
}

export function srcdoc(html: string, initialState: unknown, look: PageLook): string {
  // Meta, style and bridge come first so the parser puts them in the head, ahead of the page;
  // a page that is a full document merges into that head and body.
  return `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}"><style>${baseStyle(look)}</style><script>${bridge(initialState)}</script>${html}`
}

/**
 * The page as a file to keep: the same policy (still no network), its latest saved state,
 * and a `vizier` whose send and save do nothing, so its scripts still run outside the WebUI.
 */
export function standaloneDocument(
  html: string,
  title: string,
  state: unknown,
  look: PageLook,
  extra: { script?: string; style?: string } = {}
): string {
  const stub = `window.vizier={state:${scriptJson(state)},send:function(){},saveState:function(v){window.vizier.state=v},onState:function(){}};`
  const escapedTitle = title.replace(/&/g, '&amp;').replace(/</g, '&lt;')
  // Drawn in the WebUI the page is transparent over the app's background; on its own it gets
  // that background, so it looks the same.
  const style = `${baseStyle(look)}html{background:var(--bg);min-height:100%}body{padding:16px}${extra.style ?? ''}`
  return `<!doctype html><meta charset="utf-8"><title>${escapedTitle}</title><meta http-equiv="Content-Security-Policy" content="${CSP}"><style>${style}</style><script>${stub}${extra.script ?? ''}</script>${html}`
}

/** The message a print frame posts once its print dialog has closed. */
export const PRINTED_MESSAGE = 'vizier:printed'

/**
 * The page as a print-only document, for "Save as PDF": the standalone document, in the theme
 * and on the background it was drawn with, which opens the print dialog once it has loaded and
 * settled, then reports back. Browsers drop backgrounds when printing unless a page asks to
 * keep them, so this one does, and prints edge to edge so its background fills the page. Its
 * frame is allowed modals for the print dialog alone, so the page's own `alert`, `confirm`,
 * `prompt` and `print` are disabled before any of its scripts run.
 */
export function printDocument(html: string, title: string, state: unknown, look: PageLook): string {
  const printer = `(function(){var p=window.print.bind(window);window.alert=window.confirm=window.prompt=window.print=function(){};window.addEventListener('load',function(){setTimeout(function(){p();parent.postMessage({type:'${PRINTED_MESSAGE}'},'*')},300)})})();`
  return standaloneDocument(html, title, state, look, {
    script: printer,
    style: '*{-webkit-print-color-adjust:exact;print-color-adjust:exact}@page{margin:0}',
  })
}

export const fileName = (title: string) =>
  `${title.trim().replace(/[^a-z0-9]+/gi, '-').replace(/^-+|-+$/g, '').toLowerCase() || 'page'}.html`
