/**
 * The latency recorder for the live conversation (`specs/055-live-voice` T076/T077).
 *
 * Two numbers per turn, and both start at the same event: **the utterance closing on this client**,
 * which is the 800 ms hangover firing in `vad.ts`, not a frame arriving from the server. Timing
 * from the arrival of a frame would measure the network's answer rather than what the person
 * hears.
 *
 * - `firstAudibleMs` (SC-001): utterance close → the reply's first sample actually audible. The
 *   second half of that span comes from `player.ts`, which knows when the buffer starts and what
 *   the audio context's own latency is; it cannot be measured from here.
 * - `silenceMs` (SC-002, the client's leg): how long stopping the audio took, measured around
 *   `stopAudio()` in the same tick. The server's half — `Abort` reaching the agent within 300 ms —
 *   is not this file's business.
 *
 * This module **records; it does not judge**. There is no assertion anywhere that fails a test
 * because a percentile is over a threshold: the release gate has no agreed threshold or minimum
 * sample count yet (QC raised it), and a recorder that fails builds is a worse recorder. The
 * numbers the specs ask for are quoted below so whoever turns them into a gate has them to hand:
 * SC-001 — p90 ≤ 3.5 s, p99 ≤ 5 s · SC-002 — ≤ 300 ms in at least 95% of turns.
 *
 * Off by default, and free when off: one boolean read per turn, no object allocated, nothing
 * logged. `setLiveVoiceMetrics` is exported so a test — or a measurement run — can switch it on.
 */

/** The last turns kept for the report. A release run wants twenty; a ring is not a database. */
export const LIVE_VOICE_RING = 200

export interface TurnSample {
    /** Utterance close → the reply's first audible sample, in ms (SC-001). */
    firstAudibleMs: number
    /** Utterance close → the audio stopped, in ms (SC-002, client leg). */
    silenceMs: number
}

export interface Spread {
    p50: number
    p90: number
    p99: number
}

export interface LiveVoiceReport {
    n: number
    firstAudible: Spread
    silence: Spread
}

let enabled = false
const samples: TurnSample[] = []

/** Switch the recorder on or off. Off is the default, and off costs nothing. */
export function setLiveVoiceMetrics(on: boolean): void {
    enabled = on
}

export function liveVoiceMetricsEnabled(): boolean {
    return enabled
}

/** Forget every sample. For tests and for the start of a measurement run. */
export function resetLiveVoiceMetrics(): void {
    samples.length = 0
}

/**
 * Keep one turn's sample, and say it in the log the quickstart tells a person to read.
 *
 * One turn, one sample: the caller decides which segment of a reply is the first audible one, and
 * a second segment must not overwrite it. Nothing is kept when the recorder is off.
 */
export function recordTurn(sample: TurnSample): void {
    if (!enabled) return
    samples.push(sample)
    // A ring, not a list: the oldest goes when the newest arrives.
    if (samples.length > LIVE_VOICE_RING) samples.shift()
    logTurn(sample)
}

/** One line per turn, in the shape `quickstart.md` says to read (`live_voice_metric`). */
export function logTurn(sample: TurnSample): void {
    if (!enabled) return
    const round = (value: number) => Math.round(value * 10) / 10
    console.info(
        `live_voice_metric turn=${samples.length} end_of_utterance_to_audible_ms=${round(
            sample.firstAudibleMs
        )} end_of_utterance_to_silence_ms=${round(sample.silenceMs)}`
    )
}

/**
 * The p-th percentile by linear interpolation, `p` as a fraction (0.9 is p90).
 *
 * Linear interpolation rather than "nearest rank": with three turns in a smoke run the difference
 * is the difference between a number and a cliff, and a mean would hide the tail this is for.
 */
export function percentile(values: readonly number[], p: number): number {
    if (values.length === 0) return 0
    const sorted = [...values].sort((a, b) => a - b)
    if (sorted.length === 1) return sorted[0]
    const rank = (sorted.length - 1) * Math.min(1, Math.max(0, p))
    const low = Math.floor(rank)
    const high = Math.ceil(rank)
    if (low === high) return sorted[low]
    return sorted[low] + (sorted[high] - sorted[low]) * (rank - low)
}

const spread = (values: number[]): Spread => ({
    p50: percentile(values, 0.5),
    p90: percentile(values, 0.9),
    p99: percentile(values, 0.99),
})

/** What the samples say. An empty run reports zeros rather than throwing. */
export function liveVoiceReport(): LiveVoiceReport {
    return {
        n: samples.length,
        firstAudible: spread(samples.map((sample) => sample.firstAudibleMs)),
        silence: spread(samples.map((sample) => sample.silenceMs)),
    }
}
