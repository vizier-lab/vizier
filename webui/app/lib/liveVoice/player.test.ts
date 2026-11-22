import { describe, expect, it } from 'vitest'
import { outputLevel, SilentPlayer } from './player'

describe("the reply's level", () => {
    it('reads on the microphone meter scale: silence is nothing, speech about half, never past full', () => {
        expect(outputLevel(new Float32Array(0))).toBe(0)
        expect(outputLevel(new Float32Array(64))).toBe(0)
        expect(outputLevel(new Float32Array(64).fill(0.125))).toBeCloseTo(0.5)
        expect(outputLevel(new Float32Array(64).fill(0.9))).toBe(1)
    })

    it('is nothing from a player that plays nothing', () => {
        expect(new SilentPlayer().level()).toBe(0)
    })
})
