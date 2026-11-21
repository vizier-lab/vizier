import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { LevelMeter, WORKLET_URL } from './wavRecorder'

describe('the capture worklet', () => {
    // The desktop app's CSP refuses a blob: worklet, so it has to be a file on our own origin.
    it('is a same-origin file that registers the processor the recorder asks for', () => {
        expect(WORKLET_URL.startsWith('/')).toBe(true)
        const file = join(process.cwd(), 'public', WORKLET_URL)
        expect(existsSync(file)).toBe(true)
        expect(readFileSync(file, 'utf8')).toContain("registerProcessor('vizier-capture'")
    })
})

describe('the level meter', () => {
    it('gives one level per window, carrying a part window into the next block', () => {
        const meter = new LevelMeter(4)
        expect(meter.push(new Float32Array([0.1, 0.1, 0.1]))).toEqual([])
        const levels = meter.push(new Float32Array([0.1, 0, 0, 0, 0]))
        expect(levels).toHaveLength(2)
        expect(levels[0]).toBeCloseTo(0.4)
        expect(levels[1]).toBe(0)
    })

    it('reads silence as nothing and shouting as full, never past it', () => {
        const meter = new LevelMeter(2)
        expect(meter.push(new Float32Array([0, 0]))).toEqual([0])
        expect(meter.push(new Float32Array([0.9, -0.9]))).toEqual([1])
    })
})
