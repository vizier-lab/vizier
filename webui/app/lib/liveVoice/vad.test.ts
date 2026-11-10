import { describe, expect, it } from 'vitest'
import { EnergyVad, HANGOVER_MS, MAX_HANGOVER_MS, MIN_HANGOVER_MS, WINDOW_SAMPLES, hangoverOf, rootMeanSquare } from './vad'
import { LiveVoiceClient } from './session'
import { SilentPlayer } from './player'

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

    it('a loud room cannot raise the gate above the level speech reaches', () => {
        // The failure this closes: `MAX_FLOOR` (0.02) times the raised factor (3 x 2) is 0.12 RMS,
        // which is the band ordinary speech sits in (0.10-0.15). In a room noisy enough to hold the
        // floor at its ceiling, the person became quieter than the gate meant to hear them, and the
        // session sat in `listening` with no utterance ever ending.
        const vad = new EnergyVad()
        // 30 s of loud silence drives the adaptive floor to its 0.02 ceiling.
        feed(vad, 30_000, 0.02)
        // Ordinary speech, spoken while the reply is audible: heard, because the cap is 0.06.
        expect(vad.push(windowOf(0.1), true).voiced).toBe(true)
        // The reply's own echo is still held off: 0.02 is well under the cap.
        expect(vad.push(windowOf(0.02), true).voiced).toBe(false)
        // And the raised gate is never weaker than the unraised one, or the cap would be a bug of
        // its own: 0.03 clears the quiet gate and must stay under the raised one.
        expect(vad.push(windowOf(0.03), true).voiced).toBe(false)
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

/**
 * G1 — the hangover the session actually uses.
 *
 * It was hardcoded here and read by nobody, so the agent's `end_of_utterance_ms` never reached the
 * gate. That silence is 0.8 s of the delay a person feels before the agent even starts thinking.
 */
describe('the hangover a session uses', () => {
    it('closes the utterance at the configured silence, not the spec default', () => {
        const vad = new EnergyVad(3, hangoverOf(250))
        feed(vad, 200, 0.2)
        // 300 ms of silence: past the configured 250 ms hangover, well short of the 800 default.
        const decisions = feed(vad, 300, 0)
        expect(decisions.some((d) => d.utteranceEnd), 'the utterance ended at ~250 ms').toBe(true)
    })

    it('a gate built without an argument keeps the spec default', () => {
        const vad = new EnergyVad()
        feed(vad, 200, 0.2)
        const early = feed(vad, 400, 0)
        expect(early.some((d) => d.utteranceEnd), 'not yet at 400 ms').toBe(false)
        const late = feed(vad, 500, 0)
        expect(late.some((d) => d.utteranceEnd), 'ended past 800 ms').toBe(true)
    })

    it('clamps a setting that would make the gate unusable, and says what it clamps to', () => {
        // A zero would end every utterance on the first silent window; a huge one would leave the
        // person waiting in silence.
        expect(hangoverOf(0)).toBe(MIN_HANGOVER_MS)
        expect(hangoverOf(-50)).toBe(MIN_HANGOVER_MS)
        expect(hangoverOf(60_000)).toBe(MAX_HANGOVER_MS)
        expect(hangoverOf(250)).toBe(250)
        // Nothing configured is the spec's number, not a clamp.
        expect(hangoverOf(undefined)).toBe(HANGOVER_MS)
        expect(hangoverOf(Number.NaN)).toBe(HANGOVER_MS)
    })

    it('the configured value reaches the gate through the client', async () => {
        // The chain the setting travels: deps → client → gate. A gate that ignored it would end
        // the utterance at 800 ms whatever the agent asked for.
        const client = new LiveVoiceClient({
            send: () => true,
            player: new SilentPlayer(),
            openCapture: async () => ({ stop: () => {}, open: true }),
            onView: () => {},
            now: () => 1000,
            endOfUtteranceMs: 250,
        })
        expect(client.hangoverMs).toBe(250)
        const plain = new LiveVoiceClient({
            send: () => true,
            player: new SilentPlayer(),
            openCapture: async () => ({ stop: () => {}, open: true }),
            onView: () => {},
            now: () => 1000,
        })
        expect(plain.hangoverMs, 'an agent that says nothing gets the spec default').toBe(HANGOVER_MS)
    })
})
