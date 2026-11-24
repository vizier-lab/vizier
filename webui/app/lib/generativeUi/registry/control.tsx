import type { ComponentRenderProps } from '@json-render/react'
import { useEffect, useRef } from 'react'
import { useEmitter } from './shared'

type P = Record<string, unknown>

/** A tick-driven interface stops after this long without the person touching anything. */
export const IDLE_STOP_MS = 10 * 60 * 1000
export const MIN_INTERVAL_MS = 50

let lastInteraction = Date.now()
let listening = false

/** The time of the person's last click, key or touch anywhere on the page. */
function trackInteraction() {
    if (listening || typeof window === 'undefined') return
    listening = true
    for (const type of ['pointerdown', 'keydown', 'touchstart', 'wheel']) window.addEventListener(type, () => (lastInteraction = Date.now()), { passive: true })
}

export function shouldTick(now: number, last: number, visible: boolean, onScreen: boolean, running: boolean): boolean {
    return running && visible && onScreen && now - last < IDLE_STOP_MS
}

/**
 * Draws nothing. `load` fires once when the interface appears; with `intervalMs`, `tick` fires
 * repeatedly while the interface is on screen, the tab is visible and the person has been about.
 * A tick that is still running skips the next. Both are untrusted: they cannot send messages.
 */
export function EffectC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const send = useEmitter(emit)
    const sendRef = useRef(send)
    sendRef.current = send
    const anchor = useRef<HTMLSpanElement>(null)
    const onScreen = useRef(true)
    const interval = typeof p.intervalMs === 'number' ? Math.max(MIN_INTERVAL_MS, p.intervalMs) : null
    const running = p.running !== false

    useEffect(() => {
        trackInteraction()
        lastInteraction = Date.now()
        sendRef.current('load', null)
    }, [])

    useEffect(() => {
        const node = anchor.current
        if (!node || typeof IntersectionObserver === 'undefined') return
        const observer = new IntersectionObserver((entries) => {
            onScreen.current = entries.some((entry) => entry.isIntersecting)
        })
        observer.observe(node)
        return () => observer.disconnect()
    }, [])

    useEffect(() => {
        if (interval === null) return
        let busy = false
        const timer = setInterval(() => {
            if (busy || !shouldTick(Date.now(), lastInteraction, document.visibilityState === 'visible', onScreen.current, running)) return
            busy = true
            Promise.resolve(sendRef.current('tick', null)).finally(() => {
                busy = false
            })
        }, interval)
        return () => clearInterval(timer)
    }, [interval, running])

    return <span ref={anchor} aria-hidden hidden />
}
