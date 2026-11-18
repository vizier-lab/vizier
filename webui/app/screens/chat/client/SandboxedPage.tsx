// The one host of an agent page's frame (specs/063-html-generative-ui research R7, R7a): in a
// reply, full screen, and (in verify mode) off-screen while a new version is checked.
//
// The frame is not built here. It is a document of its own, served at a ticket URL
// (the `ui-frames/{ticket}` route) with its own policy, which sandboxes it (an opaque origin, no
// network from script) whatever this element says. `srcdoc` would inherit the desktop app's CSP,
// which runs no inline script. The frame's runtime (`app/kit/runtime.ts`) talks only through
// `postMessage`, and every rule a page must not get around is enforced here (`lib/uiPage/host.ts`).

import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { uiFrameTicket } from '../../../api/endpoints/chat'
import { transport } from '../../../api/transport'
import { SendGate, StateCoalescer, UNRESPONSIVE_MS, parseFrameMessage, themeTokens, toFrame, type Problem } from '../../../lib/uiPage/host'
import { BEATS_TO_CLEAR, clearMounting, markMounting } from '../../../lib/uiPage/mounting'
import { useAppearanceKey } from '../../../lib/useDark'
import { topicKey } from '../../../stores/connection'

const INITIAL_HEIGHT = 120
/** The shortest a page's frame gets in a reply, so a page that reports no height stays visible. */
const MIN_HEIGHT = 48

export type SandboxedPageProps = {
    agentId: string
    topicId: string
    channel?: string
    pageId: string
    version: number
    title: string
    /** `chat` draws the page; `verify` mounts it off-screen to watch it for problems. */
    mode?: 'chat' | 'verify'
    /** The page's state as it is saved now; a change from elsewhere (another tab) is posted in. */
    state?: unknown
    /** Fill the parent's height (full screen) instead of following the page's own height. */
    fill?: boolean
    /** The tallest the frame grows in a reply, in px. */
    maxHeight?: number
    onProblem?: (problem: Problem) => void
    /** The page asked to save its state (already coalesced). */
    onSaveState?: (value: unknown) => void
    /** The page sent a message as the person's turn (already gated). */
    onSend?: (text: string, data: unknown) => void
    /** A conversation file by name, as a `data:` URL. */
    resolveFile?: (name: string) => Promise<string>
    /** The frame's runtime said it is running. */
    onReady?: () => void
    className?: string
    style?: CSSProperties
}

export function SandboxedPage(props: SandboxedPageProps) {
    const { agentId, topicId, channel, pageId, version, title, mode = 'chat', fill = false } = props
    const frameRef = useRef<HTMLIFrameElement>(null)
    const [src, setSrc] = useState<string | null>(null)
    const [failed, setFailed] = useState<string | null>(null)
    const [height, setHeight] = useState(INITIAL_HEIGHT)
    const appearance = useAppearanceKey()

    // The latest callbacks and state without re-running the effects that hold the listeners.
    const latest = useRef(props)
    latest.current = props
    const gate = useRef(new SendGate())
    const ready = useRef(false)
    const lastBeat = useRef(0)
    const reportedHang = useRef(false)
    const frameHas = useRef<string | null>(null)
    const beats = useRef(0)
    /** Whether the frame is on screen: a browser slows the timers of a frame out of view, so its
     * heartbeat is not evidence of a hang then. */
    const onScreen = useRef(true)

    // A ticket per mount: it lasts ten minutes and grants only this version.
    useEffect(() => {
        let cancelled = false
        setSrc(null)
        setFailed(null)
        ready.current = false
        reportedHang.current = false
        uiFrameTicket(agentId, topicId, pageId, version, channel)
            .then((ticket) => {
                if (cancelled) return
                // Marked before the frame loads, so a version that freezes the tab is paused
                // after the reload (FR-016a); unmarked once it has beaten a few times.
                beats.current = 0
                markMounting(pageId, version, topicKey(agentId, topicId))
                setSrc(transport.frameUrl(ticket.path))
            })
            .catch((e: unknown) => {
                if (cancelled) return
                const message = e instanceof Error ? e.message : String(e)
                setFailed(message)
                latest.current.onProblem?.({ kind: 'kit', message: `The page could not be opened: ${message}`, source: 'kit' })
            })
        return () => {
            cancelled = true
            clearMounting(pageId, version)
        }
    }, [agentId, topicId, channel, pageId, version])

    const post = (type: string, body: Record<string, unknown> = {}) =>
        frameRef.current?.contentWindow?.postMessage(toFrame(type, body), '*')

    const postTheme = () => {
        const style = getComputedStyle(document.documentElement)
        const dark = document.documentElement.getAttribute('data-theme')
            ? document.documentElement.getAttribute('data-theme') === 'dark'
            : matchMedia('(prefers-color-scheme: dark)').matches
        post('theme', { tokens: themeTokens(style), dark })
    }
    const postThemeRef = useRef(postTheme)
    postThemeRef.current = postTheme
    useEffect(() => {
        if (ready.current) postThemeRef.current()
    }, [appearance])

    // A click inside the frame moves focus into it, which this window sees as its own blur.
    useEffect(() => {
        const onBlur = () =>
            setTimeout(() => {
                if (document.activeElement === frameRef.current) gate.current.interacted(Date.now())
            }, 0)
        window.addEventListener('blur', onBlur)
        return () => window.removeEventListener('blur', onBlur)
    }, [])

    useEffect(() => {
        const coalescer = new StateCoalescer((value) => {
            frameHas.current = JSON.stringify(value ?? null)
            latest.current.onSaveState?.(value)
        })
        const onMessage = (event: MessageEvent) => {
            if (!frameRef.current || event.source !== frameRef.current.contentWindow) return
            const message = parseFrameMessage(event.data)
            if (!message) return
            switch (message.type) {
                case 'ready':
                    ready.current = true
                    lastBeat.current = Date.now()
                    frameHas.current = JSON.stringify(latest.current.state ?? null)
                    postThemeRef.current()
                    latest.current.onReady?.()
                    return
                case 'heartbeat':
                    lastBeat.current = Date.now()
                    if (++beats.current === BEATS_TO_CLEAR) clearMounting(latest.current.pageId, latest.current.version)
                    return
                case 'height':
                    setHeight(Math.max(MIN_HEIGHT, message.px))
                    return
                case 'problem':
                    latest.current.onProblem?.(message.problem)
                    return
                case 'state.set':
                    coalescer.push(message.value)
                    return
                case 'send': {
                    const refused = gate.current.check(Date.now(), document.activeElement === frameRef.current)
                    if (refused || !message.text || !latest.current.onSend) {
                        post('reply', { id: message.id, ok: false, error: refused ?? (message.text ? 'not_available' : 'empty') })
                        return
                    }
                    latest.current.onSend(message.text, message.data)
                    post('reply', { id: message.id, ok: true })
                    return
                }
                case 'files.url': {
                    const resolve = latest.current.resolveFile
                    if (!resolve) {
                        post('reply', { id: message.id, ok: false, error: 'not_available' })
                        return
                    }
                    resolve(message.name).then(
                        (url) => post('reply', { id: message.id, ok: true, value: url }),
                        (e: unknown) => post('reply', { id: message.id, ok: false, error: e instanceof Error ? e.message : String(e) })
                    )
                    return
                }
            }
        }
        window.addEventListener('message', onMessage)
        // Silence after the page said it runs is a hang (FR-016a): its main thread is stuck. Only
        // while it is on screen in a visible tab (or being checked): a browser throttles the
        // timers of a frame out of view, which would read as a hang.
        const watchdog = setInterval(() => {
            if (!ready.current || reportedHang.current) return
            if (latest.current.mode !== 'verify' && (!onScreen.current || document.hidden)) {
                lastBeat.current = Date.now()
                return
            }
            if (Date.now() - lastBeat.current > UNRESPONSIVE_MS) {
                reportedHang.current = true
                latest.current.onProblem?.({ kind: 'unresponsive', message: 'The page stopped responding.', source: 'page' })
            }
        }, UNRESPONSIVE_MS / 2)
        return () => {
            window.removeEventListener('message', onMessage)
            clearInterval(watchdog)
            coalescer.dispose()
        }
    }, [])

    useEffect(() => {
        const frame = frameRef.current
        if (!frame || typeof IntersectionObserver === 'undefined') return
        const observer = new IntersectionObserver(([entry]) => {
            onScreen.current = entry.isIntersecting
            // Back in view: the beats it missed while throttled are not a hang.
            if (entry.isIntersecting) lastBeat.current = Date.now()
        })
        observer.observe(frame)
        return () => observer.disconnect()
    }, [src])

    // State saved elsewhere is posted in; the echo of this frame's own save is not.
    useEffect(() => {
        if (!ready.current) return
        const serialised = JSON.stringify(props.state ?? null)
        if (serialised === frameHas.current) return
        frameHas.current = serialised
        post('state', { value: props.state ?? null })
    }, [props.state])

    if (failed) {
        if (mode === 'verify') return null
        return (
            <div role="alert" className="px-12 py-10 type-ui text-failed-text">
                This page could not be opened: {failed}
            </div>
        )
    }

    const verify = mode === 'verify'
    const frameStyle: CSSProperties = verify
        ? { position: 'fixed', left: -20000, top: 0, width: 1024, height: 768, border: 0, visibility: 'hidden' }
        : {
              display: 'block',
              border: 0,
              width: '100%',
              background: 'transparent',
              flex: fill ? '1 1 0' : undefined,
              minHeight: fill ? 0 : undefined,
              height: fill ? undefined : `${Math.min(height, props.maxHeight ?? Math.round(window.innerHeight * 0.8))}px`,
              transition: fill ? undefined : 'height var(--vz-duration-slow) var(--vz-ease-standard)',
          }

    return src ? (
        <iframe
            ref={frameRef}
            title={title}
            src={src}
            referrerPolicy="no-referrer"
            aria-hidden={verify || undefined}
            tabIndex={verify ? -1 : undefined}
            className={props.className}
            style={{ ...frameStyle, ...props.style }}
        />
    ) : (
        <div aria-busy="true" style={verify ? { display: 'none' } : { height: INITIAL_HEIGHT }} />
    )
}
