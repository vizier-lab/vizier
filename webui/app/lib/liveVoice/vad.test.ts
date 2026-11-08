import { describe, expect, it } from 'vitest'
import { EnergyVad, HANGOVER_MS, ONSET_MS, WINDOW_SAMPLES, rootMeanSquare } from './vad'

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

    it('needs the full onset before calling a barge-in (FR-019)', () => {
        const vad = new EnergyVad()
        const decisions = feed(vad, ONSET_MS - 20, 0.05, true)
        expect(decisions.some((d) => d.bargeIn)).toBe(false)
        const more = feed(vad, 20, 0.05, true)
        expect(more[more.length - 1].bargeIn).toBe(true)
    })

    it("ignores the agent's own voice while the reply is audible (SC-008)", () => {
        // The gate is raised while playback runs, so the same level that reads as speech in
        // silence does not read as speech against the reply.
        const quiet = new EnergyVad()
        expect(quiet.push(windowOf(0.02), true).voiced).toBe(false)
        const loud = new EnergyVad()
        expect(loud.push(windowOf(0.02), false).voiced).toBe(true)
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
