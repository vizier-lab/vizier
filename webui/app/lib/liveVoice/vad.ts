/**
 * The energy gate that decides when the person started and stopped speaking
 * (`specs/055-live-voice` FR-009, FR-010, FR-026).
 *
 * No model, no wasm: a 20 ms window of RMS over an adaptive noise floor. It is enough to close an
 * utterance, and it is honest about what it cannot do — it has no notion of *who* is speaking, so
 * a second voice in the room is heard like the first (`spec.md` edge case).
 *
 * It does **not** decide when to interrupt. Since C8 the interruption happens at the end of the
 * utterance (`session.ts`), so a burst of voiced energy while the reply is audible is not a signal
 * of its own: the bytes it would have raised belong to the same utterance that will end later.
 */

/** 20 ms at 16 kHz: the window every decision is made on. */
export const WINDOW_SAMPLES = 320
/**
 * FR-010: the hangover that ends an utterance, when the agent's settings say nothing.
 *
 * Kept at the spec's number for anything that builds a gate without an argument — a test, a
 * surface with no agent behind it. The session passes the agent's own
 * `end_of_utterance_ms` instead, which is what makes the setting mean something: it used to be
 * hardcoded here and read by nobody (G1).
 */
export const HANGOVER_MS = 800
/** What a hangover is clamped to. Below 100 ms a pause for breath ends the utterance; above 2 s the
 * person waits in silence for no reason. */
export const MIN_HANGOVER_MS = 100
export const MAX_HANGOVER_MS = 2000

/** The hangover a session uses: the agent's number, clamped, or the spec's default. */
export function hangoverOf(configured?: number): number {
    if (configured === undefined || !Number.isFinite(configured)) return HANGOVER_MS
    return Math.min(MAX_HANGOVER_MS, Math.max(MIN_HANGOVER_MS, Math.round(configured)))
}
/** The floor never rises above this, so a loud room cannot deafen the gate. */
const MAX_FLOOR = 0.02
/**
 * The highest the gate may be raised to while the reply is audible, in RMS.
 *
 * Without this cap the raised gate is `floor * thresholdFactor * 2`, and `MAX_FLOOR` puts its
 * ceiling at `0.02 * 3 * 2 = 0.12` — which is the level ordinary speech reaches (0.10–0.15 RMS,
 * the same calibration `LEVEL_GAIN` in `session.ts` is built on). So in a loud room the person
 * became quieter than the gate meant to hear them: `listening` forever, no utterance end, and the
 * flag that raised the gate never cleared because only a `final` segment clears it.
 *
 * 0.06 sits between the room and the voice: above the 0.02 floor ceiling, comfortably below the
 * bottom of the speech band, and still twice the unraised gate so the reply's own echo is held
 * off. It is a cap, not a change to `thresholdFactor`: normal sensitivity is untouched.
 */
const MAX_GATE = 0.06
/** How fast the floor follows the room: slow enough that speech does not raise it. */
const FLOOR_RATE = 0.05

export interface VadDecision {
    /** The window was voiced. */
    voiced: boolean
    /** The utterance ended: the person has been silent for the whole hangover. */
    utteranceEnd: boolean
}

/** The gate and its per-utterance counters. Pure state, no audio: every method is unit-testable. */
export class EnergyVad {
    /** The adaptive noise floor, in RMS. */
    private floor = 0.005
    private silentMs = 0
    private voicedMs = 0
    private spoke = false

    constructor(
        private readonly thresholdFactor = 3,
        private readonly hangoverMs = HANGOVER_MS
    ) {}

    /** The hangover this gate ends an utterance on, in ms. */
    get hangover(): number {
        return this.hangoverMs
    }

    /** The current floor, for the level meter and for the tests. */
    get noiseFloor(): number {
        return this.floor
    }

    /** Whether the current utterance has had any voiced window yet. */
    get hasSpeech(): boolean {
        return this.spoke
    }

    /**
     * How long the current utterance has been voiced. A continuous state rather than a one-shot
     * signal, which is what reply ducking (T047) reads: it needs to know the person is speaking
     * *now*, not that they crossed a threshold once.
     */
    get voicedFor(): number {
        return this.voicedMs
    }

    /** Forget the utterance but keep the room's floor. */
    resetUtterance(): void {
        this.silentMs = 0
        this.voicedMs = 0
        this.spoke = false
    }

    /**
     * Feed one 20 ms window. `replyAudible` raises the gate while the agent is talking, so its own
     * voice from the speakers is not mistaken for the person's (FR-026, SC-008).
     */
    push(samples: Float32Array, replyAudible: boolean): VadDecision {
        const rms = rootMeanSquare(samples)
        // Capped, not merely raised: see `MAX_GATE`. An uncapped gate in a loud room is a gate that
        // cannot hear the person it exists for.
        const gate = replyAudible
            ? Math.min(this.floor * this.thresholdFactor * 2, MAX_GATE)
            : this.floor * this.thresholdFactor
        const voiced = rms > gate && rms > 0.008

        if (!voiced) {
            // The floor follows quiet audio only, and slowly: the person's own voice must not
            // raise the gate that is supposed to hear them.
            this.floor = Math.min(MAX_FLOOR, this.floor + (rms - this.floor) * FLOOR_RATE)
            this.silentMs += windowMs(samples)
            const utteranceEnd = this.spoke && this.silentMs >= this.hangoverMs
            if (utteranceEnd) this.resetUtterance()
            return { voiced: false, utteranceEnd }
        }

        this.silentMs = 0
        this.voicedMs += windowMs(samples)
        this.spoke = true
        return { voiced: true, utteranceEnd: false }
    }
}

export function rootMeanSquare(samples: Float32Array): number {
    if (samples.length === 0) return 0
    let sum = 0
    for (let i = 0; i < samples.length; i++) sum += samples[i] * samples[i]
    return Math.sqrt(sum / samples.length)
}

function windowMs(samples: Float32Array): number {
    return (samples.length / 16_000) * 1000
}

/** Cut a stream into the fixed windows the gate expects. */
export function toWindows(samples: Float32Array): Float32Array[] {
    const windows: Float32Array[] = []
    for (let i = 0; i + WINDOW_SAMPLES <= samples.length; i += WINDOW_SAMPLES) {
        windows.push(samples.subarray(i, i + WINDOW_SAMPLES))
    }
    return windows
}
