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
export const PALETTE_KEYS = ['bg', 'surface', 'border', 'text', 'muted', 'accent', 'error'] as const
export type Palette = Record<(typeof PALETTE_KEYS)[number], string>

/** The WebUI's own variable each palette colour is read from. */
export const PALETTE_SOURCES: Palette = {
  bg: '--background',
  surface: '--surface',
  border: '--border',
  text: '--text-primary',
  muted: '--text-secondary',
  accent: '--accent-primary',
  error: '--color-error',
}

// The theme's values, should a WebUI variable be missing or unusable.
const FALLBACK: Record<'dark' | 'light', Palette> = {
  dark: { bg: '#0a0a0b', surface: '#18181b', border: '#27272a', text: '#fafafa', muted: '#a1a1aa', accent: '#10B981', error: '#f87171' },
  light: { bg: '#f5f4f0', surface: '#f8f8f5', border: '#dcdad4', text: '#1a1a1a', muted: '#4a4a48', accent: '#10B981', error: '#b42318' },
}

/** How the page looks where it is drawn: the theme, and its colours. */
export interface PageLook {
  dark: boolean
  palette: Partial<Palette>
}

/** A colour that can sit inside a <style> element, or `null`. */
const safeColour = (colour: string | undefined) =>
  colour && /^[#(),.%\w\s-]+$/.test(colour) && colour.trim() ? colour.trim() : null

/**
 * Base styles: the theme's colours as variables, and defaults so an unstyled page (text,
 * links, buttons, inputs) reads as part of the reply. A page's own styles come later and win.
 */
export function baseStyle(look: PageLook): string {
  const fallback = FALLBACK[look.dark ? 'dark' : 'light']
  const vars = PALETTE_KEYS.map((key) => `--${key}:${safeColour(look.palette[key]) ?? fallback[key]}`).join(';')
  return (
    `:root{color-scheme:${look.dark ? 'dark' : 'light'};${vars};color:var(--text);font-family:system-ui,-apple-system,"Segoe UI",sans-serif;font-size:14px;line-height:1.5}` +
    'html,body{margin:0;background:transparent}a{color:var(--accent)}' +
    'button,input,select,textarea{font:inherit;color:inherit;background:var(--surface);border:1px solid var(--border);border-radius:6px;padding:4px 10px}' +
    'button{cursor:pointer}button:hover{border-color:var(--accent)}'
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
