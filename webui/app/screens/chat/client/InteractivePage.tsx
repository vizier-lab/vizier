// Draws a page the agent wrote (`render_interactive_ui`) inside its reply, sandboxed from the
// WebUI (specs/019-client-tools research R12).
//
// The isolation is the iframe's: `sandbox="allow-scripts"` without `allow-same-origin` gives
// the page an opaque origin (no token, no cookies, no parent DOM), and its CSP blocks every
// network request. What this adds is the bridge (`window.vizier`) and the rules a page cannot
// get around because they are enforced out here: messages are accepted only from this frame,
// `send` only after the person has interacted with it and at most once a second, and saved
// state is coalesced to one save per 500 ms.

import { IconButton, InteractivePage as Frame } from '@vizier/design'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { ClientCallView } from '../../../api/frames'
import type { ClientCallContext } from '../../../lib/clientTools'
import { PALETTE_SOURCES, PRINTED_MESSAGE, fileName, printDocument, srcdoc, standaloneDocument } from '../../../lib/pageDocument'
import type { PageLook, Palette } from '../../../lib/pageDocument'

const INTERACTION_WINDOW_MS = 10_000
const SEND_INTERVAL_MS = 1_000
const STATE_COALESCE_MS = 500
const INITIAL_HEIGHT = 120
const MAX_HEIGHT_RATIO = 0.8

function isDark(): boolean {
    const attr = document.documentElement.getAttribute('data-theme')
    return attr ? attr === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches
}

export function InteractivePage({ call, ctx }: { call: ClientCallView; ctx: ClientCallContext }) {
    const frameRef = useRef<HTMLIFrameElement>(null)
    const [height, setHeight] = useState(INITIAL_HEIGHT)
    const [maxHeight, setMaxHeight] = useState(() => Math.round(window.innerHeight * MAX_HEIGHT_RATIO))

    const html = typeof call.args.html === 'string' ? call.args.html : ''
    const title = typeof call.args.title === 'string' ? call.args.title : 'Interactive page'

    // The state a page starts with is baked in once; later changes are posted in, since
    // rebuilding the document would reload the page.
    const initialState = useRef(call.state)
    const look = useMemo<PageLook>(() => {
        const style = getComputedStyle(document.documentElement)
        const palette = Object.fromEntries(
            Object.entries(PALETTE_SOURCES).map(([key, source]) => [key, style.getPropertyValue(source)])
        ) as Palette
        return { dark: isDark(), palette }
    }, [])
    const document_ = useMemo(() => srcdoc(html, initialState.current, look), [html, look])

    const ctxRef = useRef(ctx)
    ctxRef.current = ctx
    const lastFocusAt = useRef(0)
    const lastSendAt = useRef(0)
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
        const onBlur = () =>
            setTimeout(() => {
                if (document.activeElement === frameRef.current) lastFocusAt.current = Date.now()
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
                    if (typeof message.height === 'number' && Number.isFinite(message.height)) setHeight(Math.max(0, Math.ceil(message.height)))
                    return
                case 'vizier:send': {
                    if (!interacted()) return
                    const now = Date.now()
                    if (now - lastSendAt.current < SEND_INTERVAL_MS) return
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

    // State saved elsewhere (another tab or browser) is posted in; the echo of this frame's
    // own save is not.
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

    // Expanded, the page covers the chat area, pinned over its rectangle rather than moved in
    // the DOM, which would reload the page.
    const wrapperRef = useRef<HTMLDivElement>(null)
    const [area, setArea] = useState<{ top: number; left: number; width: number; height: number } | null>(null)
    const measure = () => {
        const rect = wrapperRef.current?.closest('[data-chat-area]')?.getBoundingClientRect()
        return rect
            ? { top: rect.top, left: rect.left, width: rect.width, height: window.innerHeight - rect.top }
            : { top: 0, left: 0, width: window.innerWidth, height: window.innerHeight }
    }
    const fullscreen = area !== null
    useEffect(() => {
        if (!fullscreen) return
        const onResize = () => setArea(measure())
        const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setArea(null)
        window.addEventListener('resize', onResize)
        window.addEventListener('keydown', onKey)
        return () => {
            window.removeEventListener('resize', onResize)
            window.removeEventListener('keydown', onKey)
        }
    }, [fullscreen])

    // Save as PDF goes through the browser's print dialog: the page cannot be captured from
    // out here, since its frame is a different origin. It is printed from a short-lived hidden
    // frame of its own, the only one allowed the dialog.
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

    return (
        <div
            ref={wrapperRef}
            className={fullscreen ? 'fixed z-40 bg-ground p-16' : undefined}
            style={area ?? undefined}
        >
            <Frame
                title={title}
                onFullscreen={() => setArea(fullscreen ? null : measure())}
                actions={
                    <>
                        <IconButton variant="ghost" size="sm" icon="download" aria-label="Download the page as an HTML file" onClick={download} />
                        <IconButton variant="ghost" size="sm" icon="file" aria-label="Save the page as a PDF (opens the print dialog)" onClick={saveAsPdf} />
                    </>
                }
            >
                <iframe
                    ref={frameRef}
                    title={title}
                    sandbox="allow-scripts"
                    referrerPolicy="no-referrer"
                    srcDoc={document_}
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
                        colorScheme: look.dark ? 'dark' : 'light',
                        // The page reports its own height; the frame eases to it.
                        transition: fullscreen ? undefined : 'height var(--vz-duration-slow) var(--vz-ease-standard)',
                    }}
                    scrolling={fullscreen || height > maxHeight ? 'auto' : 'no'}
                />
            </Frame>
        </div>
    )
}
