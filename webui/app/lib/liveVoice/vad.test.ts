import { describe, expect, it } from 'vitest'
import { EnergyVad, HANGOVER_MS, WINDOW_SAMPLES, rootMeanSquare } from './vad'

/** One 20 ms window at a given level, as the gate sees it. */
function windowOf(level: number): Float32Array {
    const samples = new Float32Array(WINDOW_SAMPLES)
    // A square wave has a known RMS, which is what the gate measures.
    for (let i = 0; i < samples.length; i++) samples[i] = i % 2 === 0 ? level : -level
    return samples
}

/** Feed `ms` of audio at `level`, and collect every decision. */
function feed(vad: EnergyVad, ms: number, level: number, replyAudible = false) {
    const decisions = []
    for (let elapsed = 0; elapsed < ms; elapsed += 20) {
        decisions.push(vad.push(windowOf(level), replyAudible))
    }
    return decisions
}

describe('the energy gate', () => {
    it('hears speech and ends the utterance after the hangover', () => {
        const vad = new EnergyVad()
        feed(vad, 1000, 0.02)
        expect(vad.hasSpeech).toBe(true)
        // Exactly the hangover, no sooner.
        const before = feed(vad, HANGOVER_MS - 20, 0)
        expect(before.every((d) => !d.utteranceEnd)).toBe(true)
        const at = feed(vad, 20, 0)
        expect(at[at.length - 1].utteranceEnd).toBe(true)
    })

    it('does not end an utterance that never spoke', () => {
        const vad = new EnergyVad()
        const decisions = feed(vad, 2000, 0)
        expect(decisions.every((d) => !d.utteranceEnd)).toBe(true)
    })

    it("ignores the agent's own voice while the reply is audible (FR-026, SC-008)", () => {
        // The gate is doubled while playback runs. 0.02 clears the quiet gate (floor 0.005 × 3 =
        // 0.015) and falls under the raised one (× 2 = 0.03), so the same level is the person in
        // silence and the agent's own voice over the speakers (FR-026, SC-008).
        const inSilence = new EnergyVad()
        expect(inSilence.push(windowOf(0.02), false).voiced).toBe(true)
        const overReply = new EnergyVad()
        expect(overReply.push(windowOf(0.02), true).voiced).toBe(false)
        // A voice loud enough to clear the raised gate is still heard over the reply.
        expect(overReply.push(windowOf(0.2), true).voiced).toBe(true)
    })

    it('reports the voiced duration as a continuous state, which ducking reads (T047)', () => {
        const vad = new EnergyVad()
        expect(vad.voicedFor).toBe(0)
        feed(vad, 200, 0.05)
        expect(vad.voicedFor).toBeGreaterThanOrEqual(180)
        feed(vad, 900, 0)
        // The utterance closed, so the counter starts again with the next one.
        expect(vad.voicedFor).toBe(0)
    })

    it('follows a loud room without going deaf', () => {
        const vad = new EnergyVad()
        for (let i = 0; i < 200; i++) vad.push(windowOf(0.01), false)
        expect(vad.noiseFloor).toBeGreaterThan(0.005)
        // A loud room still lets a louder voice through.
        expect(vad.push(windowOf(0.2), false).voiced).toBe(true)
    })

    it('reports rms for the level meter', () => {
        expect(rootMeanSquare(new Float32Array(0))).toBe(0)
        expect(rootMeanSquare(windowOf(0.5))).toBeCloseTo(0.5, 5)
    })
})
