// Full screen for something drawn inside a reply: it is pinned over the chat area's rectangle
// rather than moved in the DOM (a sandboxed page would reload). A fixed box is not clipped by
// the shell's panel, so it takes the panel's corners where the two meet.
import { useEffect, useState, type CSSProperties, type RefObject } from 'react'

/** How far apart two edges may be and still count as the same edge. */
const SAME_EDGE = 0.5

function px(value: string): number {
    return Number.parseFloat(value) || 0
}

/** The chat area's rectangle, with the corners of the panel it sits in. */
export function measureChatArea(from: Element | null): CSSProperties {
    const area = from?.closest('[data-chat-area]')
    if (!area) return { top: 0, left: 0, width: window.innerWidth, height: window.innerHeight }
    const rect = area.getBoundingClientRect()
    const style: CSSProperties = { top: rect.top, left: rect.left, width: rect.width, height: rect.height }

    const panel = area.closest('main')
    if (!panel) return style
    const outer = panel.getBoundingClientRect()
    const css = getComputedStyle(panel)
    const border = px(css.borderTopWidth)
    const inner = (radius: string) => Math.max(0, px(radius) - border)
    if (rect.top - outer.top <= border + SAME_EDGE) {
        style.borderTopLeftRadius = inner(css.borderTopLeftRadius)
        style.borderTopRightRadius = inner(css.borderTopRightRadius)
    }
    if (outer.bottom - rect.bottom <= border + SAME_EDGE) {
        style.borderBottomLeftRadius = inner(css.borderBottomLeftRadius)
        style.borderBottomRightRadius = inner(css.borderBottomRightRadius)
    }
    return style
}

/** `area` is the style of the pinned box while open, `null` otherwise. Escape closes it. */
export function useChatAreaFullscreen(ref: RefObject<Element | null>) {
    const [area, setArea] = useState<CSSProperties | null>(null)
    const fullscreen = area !== null
    useEffect(() => {
        if (!fullscreen) return
        const onResize = () => setArea(measureChatArea(ref.current))
        const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setArea(null)
        window.addEventListener('resize', onResize)
        window.addEventListener('keydown', onKey)
        // The chat area also changes size without the window doing so (the sidebar collapsing).
        const target = ref.current?.closest('[data-chat-area]')
        const observer = target && typeof ResizeObserver !== 'undefined' ? new ResizeObserver(onResize) : null
        if (target) observer?.observe(target)
        return () => {
            window.removeEventListener('resize', onResize)
            window.removeEventListener('keydown', onKey)
            observer?.disconnect()
        }
    }, [fullscreen, ref])
    return { area, fullscreen, toggle: () => setArea(fullscreen ? null : measureChatArea(ref.current)) }
}
