import { describe, expect, it } from 'vitest'
import { clampRect, defaultRect, MIN_H, MIN_W, moved, resized } from './floatingWindow'

const VW = 1200
const VH = 800

describe('floating window geometry', () => {
    it('opens at the top right, inside the viewport', () => {
        const r = defaultRect(VW, VH)
        expect(r.x + r.w).toBeLessThanOrEqual(VW)
        expect(r.y + r.h).toBeLessThanOrEqual(VH)
        expect(r.x).toBeGreaterThan(VW / 2)
    })

    it('fits a phone-sized viewport', () => {
        const r = defaultRect(375, 667)
        expect(r.w).toBe(343)
        expect(r.x + r.w).toBeLessThanOrEqual(375)
    })

    it('moves with the pointer and never leaves the viewport', () => {
        const start = { x: 100, y: 100, w: 400, h: 300 }
        expect(moved(start, 50, -20, VW, VH)).toEqual({ x: 150, y: 80, w: 400, h: 300 })
        expect(moved(start, -500, -500, VW, VH)).toMatchObject({ x: 0, y: 0 })
        expect(moved(start, 5000, 5000, VW, VH)).toMatchObject({ x: VW - 400, y: VH - 300 })
    })

    it('resizes from the bottom-right corner and keeps the top-left', () => {
        const start = { x: 100, y: 100, w: 400, h: 300 }
        expect(resized(start, 100, 50, VW, VH)).toEqual({ x: 100, y: 100, w: 500, h: 350 })
    })

    it('has a minimum size and stops at the viewport edge', () => {
        const start = { x: 100, y: 100, w: 400, h: 300 }
        expect(resized(start, -1000, -1000, VW, VH)).toMatchObject({ w: MIN_W, h: MIN_H })
        const big = resized(start, 5000, 5000, VW, VH)
        expect(big.x + big.w).toBe(VW)
        expect(big.y + big.h).toBe(VH)
    })

    it('pulls a window back in when the viewport shrinks', () => {
        const r = clampRect({ x: 900, y: 600, w: 560, h: 520 }, 800, 600)
        expect(r.w).toBe(560)
        expect(r.x + r.w).toBeLessThanOrEqual(800)
        expect(r.y + r.h).toBeLessThanOrEqual(600)
    })
})
