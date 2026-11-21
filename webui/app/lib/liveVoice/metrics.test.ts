import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
    LIVE_VOICE_RING,
    liveVoiceReport,
    logTurn,
    percentile,
    recordTurn,
    resetLiveVoiceMetrics,
    setLiveVoiceMetrics,
} from './metrics'

beforeEach(() => {
    resetLiveVoiceMetrics()
    setLiveVoiceMetrics(true)
})

afterEach(() => {
    setLiveVoiceMetrics(false)
    resetLiveVoiceMetrics()
    vi.restoreAllMocks()
})

describe('T077 — the recorder only records', () => {
    it('says one line per turn, in the shape the quickstart tells a person to read', () => {
        const spy = vi.spyOn(console, 'info').mockImplementation(() => {})
        recordTurn({ firstAudibleMs: 1234.56, silenceMs: 12.34 })
        expect(spy).toHaveBeenCalledTimes(1)
        const line = spy.mock.calls[0][0] as string
        expect(line).toContain('live_voice_metric turn=1')
        expect(line).toContain('end_of_utterance_to_audible_ms=1234.6')
        expect(line).toContain('end_of_utterance_to_silence_ms=12.3')
    })

    it('reports both metrics over the turns it kept', () => {
        for (let i = 1; i <= 100; i += 1) recordTurn({ firstAudibleMs: i, silenceMs: i * 10 })
        const report = liveVoiceReport()
        expect(report.n).toBe(100)
        expect(report.firstAudible.p50).toBeCloseTo(50.5, 5)
        expect(report.firstAudible.p90).toBeCloseTo(90.1, 5)
        expect(report.firstAudible.p99).toBeCloseTo(99.01, 5)
        expect(report.silence.p50).toBeCloseTo(505, 5)
    })

    it('keeps a ring, not a list: the two-hundredth oldest sample is dropped', () => {
        for (let i = 1; i <= LIVE_VOICE_RING + 5; i += 1) recordTurn({ firstAudibleMs: i, silenceMs: 0 })
        const report = liveVoiceReport()
        expect(report.n).toBe(LIVE_VOICE_RING)
        // Samples 1..5 went, so the ring holds 6..205: the median sits between the hundredth and
        // the hundred-and-first of those, 105.5 — not the 102.5 the whole run would have given.
        expect(report.firstAudible.p50).toBeCloseTo(105.5, 5)
    })

    it('an empty run reports zeros instead of throwing', () => {
        expect(() => liveVoiceReport()).not.toThrow()
        expect(liveVoiceReport()).toEqual({
            n: 0,
            firstAudible: { p50: 0, p90: 0, p99: 0 },
            silence: { p50: 0, p90: 0, p99: 0 },
        })
    })

    it('costs nothing when it is off: nothing kept, nothing logged', () => {
        const spy = vi.spyOn(console, 'info').mockImplementation(() => {})
        setLiveVoiceMetrics(false)
        recordTurn({ firstAudibleMs: 10, silenceMs: 20 })
        logTurn({ firstAudibleMs: 10, silenceMs: 20 })
        expect(spy).not.toHaveBeenCalled()
        expect(liveVoiceReport().n).toBe(0)
    })

    it('is no gate: a turn far over SC-001 is recorded and nothing fails', () => {
        // SC-001 asks p90 <= 3.5 s, p99 <= 5 s. A recorder that turned those into an assertion
        // would fail this test on purpose; it must not, because the threshold is not ours to set.
        recordTurn({ firstAudibleMs: 30_000, silenceMs: 900 })
        expect(liveVoiceReport().firstAudible.p99).toBe(30_000)
    })
})

describe('percentile, by linear interpolation', () => {
    it('one sample is that sample', () => {
        expect(percentile([42], 0.5)).toBe(42)
        expect(percentile([42], 0.99)).toBe(42)
    })

    it('two samples interpolate between them', () => {
        expect(percentile([10, 20], 0.5)).toBe(15)
        expect(percentile([10, 20], 0)).toBe(10)
        expect(percentile([10, 20], 1)).toBe(20)
    })

    it('a hundred samples land where interpolation says they do', () => {
        const values = Array.from({ length: 100 }, (_, i) => i + 1)
        expect(percentile(values, 0.5)).toBeCloseTo(50.5, 6)
        expect(percentile(values, 0.9)).toBeCloseTo(90.1, 6)
        expect(percentile(values, 0.99)).toBeCloseTo(99.01, 6)
    })

    it('no samples is zero, not a throw and not NaN', () => {
        expect(percentile([], 0.9)).toBe(0)
    })

    it('does not depend on the order it is given', () => {
        expect(percentile([3, 1, 2], 0.5)).toBe(2)
        expect(percentile([2, 3, 1], 0.5)).toBe(2)
    })
})
