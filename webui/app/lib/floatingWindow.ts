/**
 * Geometry of a floating window that is moved by its header and resized by a corner handle.
 * Pure, so the clamping that keeps the header reachable is tested without a browser.
 */

export type Rect = { x: number; y: number; w: number; h: number }

export const MIN_W = 320
export const MIN_H = 240

/** Keeps the window inside the viewport and at least `MIN_W` x `MIN_H` (or the viewport, if smaller). */
export function clampRect(r: Rect, vw: number, vh: number): Rect {
    const w = Math.min(Math.max(r.w, Math.min(MIN_W, vw)), vw)
    const h = Math.min(Math.max(r.h, Math.min(MIN_H, vh)), vh)
    const x = Math.min(Math.max(r.x, 0), Math.max(0, vw - w))
    const y = Math.min(Math.max(r.y, 0), Math.max(0, vh - h))
    return { x, y, w, h }
}

/** Top right, beside the conversation, leaving the composer clear. */
export function defaultRect(vw: number, vh: number): Rect {
    const w = Math.min(560, vw - 32)
    const h = Math.min(400, vh - 120)
    return clampRect({ x: vw - w - 24, y: 72, w, h }, vw, vh)
}

/** The window dragged by (`dx`, `dy`) from where it was when the drag began. */
export function moved(start: Rect, dx: number, dy: number, vw: number, vh: number): Rect {
    return clampRect({ ...start, x: start.x + dx, y: start.y + dy }, vw, vh)
}

/** The window resized by (`dx`, `dy`) from its bottom-right corner; the top-left stays put. */
export function resized(start: Rect, dx: number, dy: number, vw: number, vh: number): Rect {
    const w = Math.max(Math.min(MIN_W, vw), Math.min(start.w + dx, vw - start.x))
    const h = Math.max(Math.min(MIN_H, vh), Math.min(start.h + dy, vh - start.y))
    return clampRect({ ...start, w, h }, vw, vh)
}
