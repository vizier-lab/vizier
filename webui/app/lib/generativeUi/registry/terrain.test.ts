// lint-tokens-exempt: palette stops in hex, as the Heightfield palette prop takes them.
import { describe, expect, it } from 'vitest'
import { colourAt, heightAt, heightfieldGeometry } from './terrain'

const flatGrid = (n: number, h = 0.5) => Array.from({ length: n * n }, () => h)

describe('heightfieldGeometry', () => {
    it('is low-poly by default: two triangles per cell with their own vertices', () => {
        const g = heightfieldGeometry({ size: [10, 10], resolution: 4, heights: flatGrid(4) })
        expect(g.indices).toBeNull()
        expect(g.positions.length / 9).toBe(3 * 3 * 2)
        expect(g.colors.length).toBe(g.positions.length)
    })

    it('shares vertices when flat is false', () => {
        const g = heightfieldGeometry({ size: [10, 10], resolution: 4, heights: flatGrid(4), flat: false })
        expect(g.positions.length / 3).toBe(16)
        expect(g.indices?.length).toBe(3 * 3 * 6)
        expect(Math.max(...g.indices!)).toBeLessThan(16)
    })

    it('centres the grid on the origin and scales heights', () => {
        const g = heightfieldGeometry({ size: [10, 20], resolution: 2, heights: [0, 1, 0, 0], heightScale: 3, flat: false })
        const xs = [...Array(4).keys()].map((i) => g.positions[i * 3])
        const zs = [...Array(4).keys()].map((i) => g.positions[i * 3 + 2])
        expect(Math.min(...xs)).toBe(-5)
        expect(Math.max(...xs)).toBe(5)
        expect(Math.min(...zs)).toBe(-10)
        expect(Math.max(...zs)).toBe(10)
        expect(g.positions[1 * 3 + 1]).toBe(3)
    })

    it('treats missing or non-finite heights as zero and clamps the resolution', () => {
        const g = heightfieldGeometry({ size: [1, 1], resolution: 3, heights: [NaN, 1], flat: false })
        expect(g.positions[1]).toBe(0)
        expect([...g.positions].every(Number.isFinite)).toBe(true)
        expect(heightfieldGeometry({ size: [1, 1], resolution: 1000, heights: [], flat: false }).positions.length / 3).toBe(192 * 192)
    })

    it('colours a face by its average height', () => {
        const palette = [{ at: 0, color: '#000000' }, { at: 1, color: '#ffffff' }]
        const g = heightfieldGeometry({ size: [1, 1], resolution: 2, heights: [1, 1, 1, 1], palette })
        expect(g.colors[0]).toBeCloseTo(1)
        const low = heightfieldGeometry({ size: [1, 1], resolution: 2, heights: [0, 0, 0, 0], palette })
        expect(low.colors[0]).toBeCloseTo(0)
    })
})

describe('colourAt', () => {
    const palette = [{ at: 0, color: '#000000' }, { at: 0.5, color: '#ff0000' }, { at: 1, color: '#ffffff' }]

    it('interpolates between stops and clamps outside them', () => {
        expect(colourAt(palette, 0.25)[0]).toBeCloseTo(0.5)
        expect(colourAt(palette, -3)).toEqual([0, 0, 0])
        expect(colourAt(palette, 9)).toEqual([1, 1, 1])
        expect(colourAt(palette, NaN)).toEqual([0, 0, 0])
    })
})

describe('heightAt', () => {
    it('reads the nearest sample', () => {
        const heights = [0, 0.25, 0.5, 1]
        expect(heightAt({ size: [2, 2], resolution: 2, heights }, 1, 1)).toBe(1)
        expect(heightAt({ size: [2, 2], resolution: 2, heights }, -1, -1)).toBe(0)
        expect(heightAt({ size: [2, 2], resolution: 2, heights }, 99, 99)).toBe(1)
    })
})
