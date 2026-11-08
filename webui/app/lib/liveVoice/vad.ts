/**
 * The energy gate that decides when the person started and stopped speaking
 * (`specs/055-live-voice` FR-009, FR-010, FR-019, FR-026).
 *
 * No model, no wasm: a 20 ms window of RMS over an adaptive noise floor. It is enough to end an
 * utterance and to detect a barge-in, and it is honest about what it cannot do — it has no notion
 * of *who* is speaking, so a second voice in the room is heard like the first (`spec.md` edge case).
 */

/** 20 ms at 16 kHz: the window every decision is made on. */
export const WINDOW_SAMPLES = 320
/** FR-010: the hangover that ends an utterance. */
export const HANGOVER_MS = 800
/** FR-019: how much voiced energy before a barge-in signal is sent, so a cough is not one. */
export const ONSET_MS = 250
/** The floor never rises above this, so a loud room cannot deafen the gate. */
const MAX_FLOOR = 0.02
/** How fast the floor follows the room: slow enough that speech does not raise it. */
const FLOOR_RATE = 0.05

export interface VadDecision {
    /** The window was voiced. */
    voiced: boolean
    /** The utterance ended: the person has been silent for the whole hangover. */
    utteranceEnd: boolean
    /** A barge-in signal is due: enough voiced energy while the agent was speaking. */
    bargeIn: boolean
}

/** The gate and its per-utterance counters. Pure state, no audio: every method is unit-testable. */
export class EnergyVad {
    /** The adaptive noise floor, in RMS. */
    private floor = 0.005
    private silentMs = 0
    private voicedMs = 0
    private onsetMs = 0
    private spoke = false

    constructor(
        private readonly thresholdFactor = 3,
        private readonly hangoverMs = HANGOVER_MS,
        private readonly onsetMs_ = ONSET_MS
    ) {}

    /** The current floor, for the level meter and for the tests. */
    get noiseFloor(): number {
        return this.floor
    }

    /** Whether the current utterance has had any voiced window yet. */
    get hasSpeech(): boolean {
        return this.spoke
    }

    /** Forget the utterance but keep the room's floor. */
    resetUtterance(): void {
        this.silentMs = 0
        this.voicedMs = 0
        this.onsetMs = 0
        this.spoke = false
    }

    /**
     * Feed one 20 ms window. `replyAudible` raises the gate while the agent is talking, so its own
     * voice from the speakers is not mistaken for a barge-in (FR-026, SC-008).
     */
    push(samples: Float32Array, replyAudible: boolean): VadDecision {
        const rms = rootMeanSquare(samples)
        const gate = this.floor * (replyAudible ? this.thresholdFactor * 2 : this.thresholdFactor)
        const voiced = rms > gate && rms > 0.008

        if (!voiced) {
            // The floor follows quiet audio only, and slowly: the person's own voice must not
            // raise the gate that is supposed to hear them.
            this.floor = Math.min(MAX_FLOOR, this.floor + (rms - this.floor) * FLOOR_RATE)
            this.silentMs += windowMs(samples)
            this.onsetMs = 0
            const utteranceEnd = this.spoke && this.silentMs >= this.hangoverMs
            if (utteranceEnd) this.resetUtterance()
            return { voiced: false, utteranceEnd, bargeIn: false }
        }

        this.silentMs = 0
        this.voicedMs += windowMs(samples)
        let bargeIn = false
        if (replyAudible) {
            this.onsetMs += windowMs(samples)
            if (this.onsetMs >= this.onsetMs_) {
                bargeIn = true
                this.onsetMs = 0
            }
        }
        this.spoke = true
        return { voiced: true, utteranceEnd: false, bargeIn }
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
