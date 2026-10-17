// Draws a page the agent wrote (`render_interactive_ui`) inside its reply, sandboxed from the
// WebUI (specs/019-client-tools research R12, contracts/webui-extension-api.md).
//
// The isolation is the iframe's, not this component's: `sandbox="allow-scripts"` without
// `allow-same-origin` gives the page an opaque origin (no `localStorage` auth token, no
// cookies, no parent DOM), no forms, popups, dialogs or navigation; the CSP meta, parsed
// before any agent markup, blocks every network request. What this component adds is the
// bridge (`window.vizier`), and the rules a page cannot get around because they are enforced
// out here: messages are accepted only from this frame, `send` only after the person has
// interacted with it and at most once a second, saved state coalesced to one save per 500 ms.

import { useEffect, useMemo, useRef, useState } from 'react'
import type { ClientCallContext } from '../../lib/clientTools'
import type { ClientCallView } from '../../interfaces/types'

const CSP =
  "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; media-src data:"

/** How long after the person was last in the frame it may still send. */
const INTERACTION_WINDOW_MS = 10_000
const SEND_INTERVAL_MS = 1_000
const STATE_COALESCE_MS = 500
const INITIAL_HEIGHT = 120
const MAX_HEIGHT_RATIO = 0.8

// JSON that can sit inside a <script> element.
const scriptJson = (value: unknown) =>
  JSON.stringify(value ?? null).replace(/</g, '\\u003c')

function bridge(initialState: unknown): string {
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

/** Base styles so an unstyled page reads as part of the reply in either theme. */
function baseStyle(dark: boolean): string {
  return `:root{color-scheme:${dark ? 'dark' : 'light'};color:${dark ? '#e5e7eb' : '#111827'};font-family:system-ui,-apple-system,"Segoe UI",sans-serif;font-size:14px;line-height:1.5}html,body{margin:0;background:transparent}`
}

function srcdoc(html: string, initialState: unknown, dark: boolean): string {
  // Meta, style and bridge come first so the parser puts them in the head, ahead of the page;
  // a page that is a full document merges into that head and body.
  return `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}"><style>${baseStyle(dark)}</style><script>${bridge(initialState)}</script>${html}`
}

interface InteractivePageProps {
  call: ClientCallView
  ctx: ClientCallContext
}

export function InteractivePage({ call, ctx }: InteractivePageProps) {
  const frameRef = useRef<HTMLIFrameElement>(null)
  const [height, setHeight] = useState(INITIAL_HEIGHT)
  const [maxHeight, setMaxHeight] = useState(() => Math.round(window.innerHeight * MAX_HEIGHT_RATIO))

  const html = typeof call.args.html === 'string' ? call.args.html : ''
  const title = typeof call.args.title === 'string' ? call.args.title : 'Interactive page'

  // The state a page starts with is baked into its document once; later changes are posted
  // in, since rebuilding the document would reload the page.
  const initialState = useRef(call.state)
  const dark = useMemo(() => document.documentElement.classList.contains('dark'), [])
  const document_ = useMemo(() => srcdoc(html, initialState.current, dark), [html, dark])

  const ctxRef = useRef(ctx)
  ctxRef.current = ctx
  const lastFocusAt = useRef(0)
  const lastSendAt = useRef(0)
  // The state the page itself holds, serialised: baked in on load, then whatever it saved or
  // was sent. `null` until it has loaded, since a message posted before then is lost.
  const frameHas = useRef<string | null>(null)
  const pendingState = useRef<{ value: unknown } | null>(null)
  const stateTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    const onResize = () => setMaxHeight(Math.round(window.innerHeight * MAX_HEIGHT_RATIO))
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])

  // A click inside the frame moves focus into it, which this window sees as its own blur.
  // Leaving the frame starts the window in which a send still counts as the person's.
  useEffect(() => {
    const inFrame = () => document.activeElement === frameRef.current
    const onBlur = () =>
      setTimeout(() => {
        if (inFrame()) lastFocusAt.current = Date.now()
      }, 0)
    const onFocus = () => {
      if (lastFocusAt.current > 0) lastFocusAt.current = Date.now()
    }
    window.addEventListener('blur', onBlur)
    window.addEventListener('focus', onFocus)
    return () => {
      window.removeEventListener('blur', onBlur)
      window.removeEventListener('focus', onFocus)
    }
  }, [])

  useEffect(() => {
    const interacted = () =>
      document.activeElement === frameRef.current ||
      (lastFocusAt.current > 0 && Date.now() - lastFocusAt.current < INTERACTION_WINDOW_MS)

    const onMessage = (event: MessageEvent) => {
      if (!frameRef.current || event.source !== frameRef.current.contentWindow) return
      const message = event.data
      if (!message || typeof message !== 'object') return

      switch (message.type) {
        case 'vizier:height':
          if (typeof message.height === 'number' && Number.isFinite(message.height)) {
            setHeight(Math.max(0, Math.ceil(message.height)))
          }
          return
        case 'vizier:send': {
          if (!interacted()) {
            console.warn('Interactive page: send dropped, the person has not interacted with the page')
            return
          }
          const now = Date.now()
          if (now - lastSendAt.current < SEND_INTERVAL_MS) {
            console.warn('Interactive page: send dropped, at most one per second')
            return
          }
          const text = typeof message.text === 'string' ? message.text.trim() : ''
          if (!text) return
          lastSendAt.current = now
          ctxRef.current.sendEvent(text, message.data)
          return
        }
        case 'vizier:state':
          pendingState.current = { value: message.value }
          if (stateTimer.current) return
          stateTimer.current = setTimeout(() => {
            stateTimer.current = null
            const pending = pendingState.current
            pendingState.current = null
            if (!pending) return
            frameHas.current = JSON.stringify(pending.value ?? null)
            ctxRef.current.saveState(pending.value)
          }, STATE_COALESCE_MS)
          return
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [])

  useEffect(
    () => () => {
      if (stateTimer.current) clearTimeout(stateTimer.current)
    },
    []
  )

  // State saved elsewhere (another tab, another browser) is posted in; the echo of this
  // frame's own save is not.
  const postState = () => {
    if (frameHas.current === null) return
    const serialised = JSON.stringify(call.state ?? null)
    if (serialised === frameHas.current) return
    frameHas.current = serialised
    frameRef.current?.contentWindow?.postMessage({ type: 'vizier:state', value: call.state ?? null }, '*')
  }
  const postStateRef = useRef(postState)
  postStateRef.current = postState
  useEffect(() => {
    postStateRef.current()
  }, [call.state])

  return (
    <iframe
      ref={frameRef}
      title={title}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      srcDoc={document_}
      // A state change that landed before the page loaded is posted once it has.
      onLoad={() => {
        frameHas.current = JSON.stringify(initialState.current ?? null)
        postStateRef.current()
      }}
      style={{
        display: 'block',
        border: 0,
        width: '100%',
        height: `${Math.min(height, maxHeight)}px`,
        background: 'transparent',
        // The same scheme as the page's own, or the browser paints an opaque backdrop.
        colorScheme: dark ? 'dark' : 'light',
      }}
      scrolling={height > maxHeight ? 'auto' : 'no'}
    />
  )
}

export default InteractivePage
