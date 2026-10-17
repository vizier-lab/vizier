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
import { FaCompress, FaDownload, FaExpand, FaFilePdf } from 'react-icons/fa6'
import type { ClientCallContext } from '../../lib/clientTools'
import type { ClientCallView } from '../../interfaces/types'
import {
  PALETTE_SOURCES,
  PRINTED_MESSAGE,
  fileName,
  printDocument,
  srcdoc,
  standaloneDocument,
} from '../../lib/pageDocument'
import type { Palette, PageLook } from '../../lib/pageDocument'

/** How long after the person was last in the frame it may still send. */
const INTERACTION_WINDOW_MS = 10_000
const SEND_INTERVAL_MS = 1_000
const STATE_COALESCE_MS = 500
const INITIAL_HEIGHT = 120
const MAX_HEIGHT_RATIO = 0.8

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
  // The WebUI's theme as the page is first drawn: its colours become the page's variables.
  const look = useMemo<PageLook>(() => {
    const style = getComputedStyle(document.documentElement)
    const palette = Object.fromEntries(
      Object.entries(PALETTE_SOURCES).map(([key, source]) => [key, style.getPropertyValue(source)])
    ) as Palette
    return { dark: document.documentElement.classList.contains('dark'), palette }
  }, [])
  const dark = look.dark
  const document_ = useMemo(() => srcdoc(html, initialState.current, look), [html, look])

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

  const wrapperRef = useRef<HTMLDivElement>(null)
  // Expanded, the page covers the chat area (the messages and the input below them), leaving
  // the app's sidebar and header in view. It is pinned over that area's rectangle rather than
  // moved in the DOM, which would reload the page.
  const [area, setArea] = useState<{ top: number; left: number; width: number; height: number } | null>(null)
  const fullscreen = area !== null
  const measure = () => {
    const chat = wrapperRef.current?.closest('.main-body')
    const rect = chat?.getBoundingClientRect()
    return rect
      ? { top: rect.top, left: rect.left, width: rect.width, height: window.innerHeight - rect.top }
      : { top: 0, left: 0, width: window.innerWidth, height: window.innerHeight }
  }
  useEffect(() => {
    if (!fullscreen) return
    const onResize = () => setArea(measure())
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setArea(null)
    }
    window.addEventListener('resize', onResize)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('resize', onResize)
      window.removeEventListener('keydown', onKey)
    }
  }, [fullscreen])

  const toggleFullscreen = () => setArea(fullscreen ? null : measure())

  // Save as PDF goes through the browser's print dialog: the page cannot be captured from out
  // here, since its frame is a different origin. It is printed from a short-lived hidden frame
  // of its own, the only one allowed the dialog; the page's frame never is.
  const saveAsPdf = () => {
    const frame = document.createElement('iframe')
    frame.setAttribute('sandbox', 'allow-scripts allow-modals')
    frame.setAttribute('referrerpolicy', 'no-referrer')
    frame.setAttribute('aria-hidden', 'true')
    frame.title = `${title} (print)`
    frame.style.cssText = 'position:fixed;left:-10000px;top:0;width:800px;height:600px;border:0'
    let removed = false
    const remove = () => {
      if (removed) return
      removed = true
      window.removeEventListener('message', onPrinted)
      frame.remove()
    }
    const onPrinted = (event: MessageEvent) => {
      if (event.source === frame.contentWindow && event.data?.type === PRINTED_MESSAGE) remove()
    }
    window.addEventListener('message', onPrinted)
    // A print dialog left open blocks that frame, not this page; the frame goes eventually.
    setTimeout(remove, 10 * 60 * 1000)
    frame.srcdoc = printDocument(html, title, call.state ?? null, look)
    document.body.appendChild(frame)
  }

  const download = () => {
    const blob = new Blob([standaloneDocument(html, title, call.state ?? null, look)], { type: 'text/html' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = fileName(title)
    link.click()
    setTimeout(() => URL.revokeObjectURL(url), 0)
  }

  const button =
    'inline-flex h-7 w-7 items-center justify-center rounded border-0 bg-transparent cursor-pointer'

  return (
    <div
      ref={wrapperRef}
      className="group relative"
      style={
        area
          ? {
              position: 'fixed',
              ...area,
              // Below the toasts (z-50), so a page's sends are announced over it.
              zIndex: 40,
              background: 'var(--background)',
              padding: '16px',
            }
          : undefined
      }
    >
      {/* Shown on hover or keyboard focus, so the page still reads as part of the reply. */}
      <div
        className="absolute right-1 top-1 z-10 flex gap-1 rounded opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100"
        style={{ background: 'var(--surface)', boxShadow: 'var(--shadow-sm)' }}
      >
        <button
          type="button"
          className={button}
          style={{ color: 'var(--text-secondary)' }}
          onClick={toggleFullscreen}
          title={fullscreen ? 'Collapse' : 'Expand'}
          aria-label={fullscreen ? 'Collapse the page' : 'Expand the page over the chat'}
        >
          {fullscreen ? <FaCompress size={12} /> : <FaExpand size={12} />}
        </button>
        <button
          type="button"
          className={button}
          style={{ color: 'var(--text-secondary)' }}
          onClick={download}
          title="Download page"
          aria-label="Download the page as an HTML file"
        >
          <FaDownload size={12} />
        </button>
        <button
          type="button"
          className={button}
          style={{ color: 'var(--text-secondary)' }}
          onClick={saveAsPdf}
          title="Save as PDF"
          aria-label="Save the page as a PDF (opens the print dialog)"
        >
          <FaFilePdf size={12} />
        </button>
      </div>
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
          height: fullscreen ? '100%' : `${Math.min(height, maxHeight)}px`,
          background: 'transparent',
          // The same scheme as the page's own, or the browser paints an opaque backdrop.
          colorScheme: dark ? 'dark' : 'light',
        }}
        scrolling={fullscreen || height > maxHeight ? 'auto' : 'no'}
      />
    </div>
  )
}

export default InteractivePage
