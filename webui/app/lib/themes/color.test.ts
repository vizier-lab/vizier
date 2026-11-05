// lint-tokens-exempt: measured values.
import { describe, expect, it } from 'vitest'
import { contrast, deltaEok, ensureContrast, hexToOklch, isHex, mix, normalizeHex, oklchToHex, rotateHue } from './color'

describe('hex', () => {
    it('accepts only opaque six-digit hex', () => {
        expect(isHex('#1e1e2e')).toBe(true)
        expect(isHex('#1E1E2E')).toBe(true)
        for (const bad of ['#fff', '#12345678', 'red', 'url(x)', '#12345g', '', null, 5, 'red; background:url(x)']) {
            expect(isHex(bad)).toBe(false)
        }
    })

    it('expands what a person types', () => {
        expect(normalizeHex(' #ABC ')).toBe('#aabbcc')
        expect(normalizeHex('#AABBCC')).toBe('#aabbcc')
        expect(normalizeHex('abc')).toBeNull()
        expect(normalizeHex('url(javascript:alert(1))')).toBeNull()
    })
})

describe('oklch', () => {
    it('round trips', () => {
        for (const hex of ['#ffffff', '#000000', '#059669', '#c8371d', '#5b7be8', '#1e1e2e']) {
            expect(oklchToHex(hexToOklch(hex))).toBe(hex)
        }
    })

    it('stays in gamut when a colour is impossible', () => {
        expect(isHex(oklchToHex({ l: 0.9, c: 0.4, h: 30 }))).toBe(true)
    })

    it('mixes greys without picking up a hue', () => {
        const middle = hexToOklch(mix('#ffffff', '#000000', 0.5))
        expect(middle.c).toBeLessThan(0.001)
    })

    it('takes the shorter way round the hue', () => {
        const { h } = hexToOklch(mix(oklchToHex({ l: 0.6, c: 0.1, h: 350 }), oklchToHex({ l: 0.6, c: 0.1, h: 10 }), 0.5))
        expect(Math.min(h, 360 - h)).toBeLessThan(3)
    })

    it('keeps the ends of a mix', () => {
        expect(mix('#059669', '#c8371d', 0)).toBe('#059669')
        expect(mix('#059669', '#c8371d', 1)).toBe('#c8371d')
    })

    it('rotates a grey only when given a chroma', () => {
        expect(hexToOklch(rotateHue('#808080', 90)).c).toBeLessThan(0.001)
        expect(hexToOklch(rotateHue('#808080', 90, 0.1)).c).toBeGreaterThan(0.09)
    })
})

describe('contrast', () => {
    it('matches the ends of the scale', () => {
        expect(contrast('#ffffff', '#000000')).toBeCloseTo(21, 5)
        expect(contrast('#123456', '#123456')).toBeCloseTo(1, 5)
    })

    // Research R10, measured against bg/surface.
    it.each([
        ['#111114', '#f4f4f5', 17.15],
        ['#2e2e33', '#f4f4f5', 12.29],
        ['#3f3f46', '#f4f4f5', 9.5],
        ['#5b5b66', '#f4f4f5', 6.1],
        ['#046c4e', '#f4f4f5', 5.86],
        ['#6e6e78', '#f4f4f5', 4.59],
        ['#85858f', '#f4f4f5', 3.32],
        ['#ffffff', '#151518', 18.22],
        ['#ededef', '#151518', 15.59],
        ['#d6d6dc', '#151518', 12.59],
        ['#a8a8b2', '#151518', 7.73],
        ['#10b981', '#151518', 7.18],
        ['#82828c', '#151518', 4.79],
        ['#6e6e78', '#151518', 3.61],
    ])('%s on %s is %f:1', (fg, bg, ratio) => {
        expect(contrast(fg, bg)).toBeCloseTo(ratio, 1)
    })

    it('moves a colour only as far as it has to', () => {
        const fixed = ensureContrast('#42be65', '#ffffff', 3)
        expect(contrast(fixed, '#ffffff')).toBeGreaterThanOrEqual(3)
        expect(contrast(fixed, '#ffffff')).toBeLessThan(3.2)
        expect(ensureContrast('#111114', '#ffffff', 4.5)).toBe('#111114')
    })

    it('goes whichever way can reach the ratio', () => {
        expect(contrast(ensureContrast('#303030', '#101010', 4.5), '#101010')).toBeGreaterThanOrEqual(4.5)
        expect(contrast(ensureContrast('#c0c0c0', '#ffffff', 4.5), '#ffffff')).toBeGreaterThanOrEqual(4.5)
    })
})

describe('deltaEok', () => {
    // Research R8b.
    it.each([
        ['#111114', '#059669', 0.437],
        ['#f2f2f5', '#10b981', 0.306],
        ['#bd93f9', '#50fa7b', 0.381],
        ['#cba6f7', '#a6e3a1', 0.236],
        ['#fe8019', '#b8bb26', 0.173],
        ['#88c0d0', '#a3be8c', 0.095],
    ])('%s to %s is %f', (a, b, distance) => {
        expect(deltaEok(a, b)).toBeCloseTo(distance, 2)
    })

    it('is zero for one colour', () => {
        expect(deltaEok('#a7c080', '#a7c080')).toBe(0)
    })
})
