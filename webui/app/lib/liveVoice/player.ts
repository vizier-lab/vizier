/**
 * Gapless playback of the reply's segments (`specs/055-live-voice` FR-022, SC-002).
 *
 * One `AudioContext`, one ordered queue, each buffer scheduled on the context clock so a segment
 * starts where the last ended. `stop()` closes every source in under 50 ms — that is the leg of
 * SC-002 the client owns, and it is why the audio stops before the server is told anything.
 */

/** When a queued segment will be heard, from the moment it was handed over (T076, SC-001). */
export interface Queued {
    /**
     * Milliseconds from this call until the first sample reaches the speaker: the wait for its
     * turn on the context clock plus the context's own output latency. It is a *forecast* the
     * context gives us — `baseLatency` and `outputLatency` — not a stopwatch reading, because the
     * page cannot observe the speaker itself.
     */
    audibleInMs: number
}

export interface Player {
    /** Queue one segment; it plays after everything already queued, and says when it starts. */
    enqueue(samples: Float32Array, sampleRate: number, final: boolean): Queued
    /** Stop now: every queued and playing source is closed and the queue is drained. */
    stop(): void
    /** Whether anything is queued or playing. */
    get playing(): boolean
    /** Resolve the context on a user gesture; browsers refuse audio before one. */
    resume(): Promise<void>
    close(): Promise<void>
}

/** The real player: an `AudioContext` and one `AudioBufferSourceNode` per segment. */
export class WebAudioPlayer implements Player {
    private context: AudioContext
    private readonly pending: AudioBufferSourceNode[] = []
    private nextStart = 0

    constructor(context?: AudioContext) {
        this.context = context ?? WebAudioPlayer.newContext()
    }

    private static newContext(): AudioContext {
        return new AudioContext({ sampleRate: 48_000 })
    }

    /**
     * The context to play on.
     *
     * `close()` is what releases the audio device, and a closed context cannot be resumed, so a
     * session that ends and starts again in the same client needs a fresh one. Recreating it here
     * keeps both halves true: every ending really closes the device, and the next call still
     * works without rebuilding the client.
     */
    private liveContext(): AudioContext {
        if (this.context.state === 'closed') this.context = WebAudioPlayer.newContext()
        return this.context
    }

    get playing(): boolean {
        return this.pending.length > 0
    }

    async resume(): Promise<void> {
        const context = this.liveContext()
        if (context.state !== 'running') await context.resume()
    }

    enqueue(samples: Float32Array, sampleRate: number, _final: boolean): Queued {
        if (samples.length === 0) return { audibleInMs: 0 }
        const context = this.liveContext()
        const buffer = context.createBuffer(1, samples.length, sampleRate)
        // Copied into a plain Float32Array: `copyToChannel` does not take a possibly-shared view.
        buffer.copyToChannel(new Float32Array(samples), 0)
        const source = context.createBufferSource()
        source.buffer = buffer
        source.connect(context.destination)

        const now = context.currentTime
        // Back to back on the context clock: a segment starts where the previous one ends, so
        // there is no gap between them.
        const startAt = Math.max(now, this.nextStart)
        source.start(startAt)
        this.nextStart = startAt + buffer.duration
        this.pending.push(source)
        source.onended = () => {
            const at = this.pending.indexOf(source)
            if (at >= 0) this.pending.splice(at, 1)
            if (this.pending.length === 0) this.nextStart = 0
        }
        // The wait for this segment's turn on the context clock, plus what the context says its
        // own output costs. Both latency figures are absent in some browsers; absent means
        // unknown, and zero is the honest floor rather than a guess.
        const latency = (context.baseLatency ?? 0) + (context.outputLatency ?? 0)
        return { audibleInMs: Math.max(0, (startAt - now) * 1000 + latency * 1000) }
    }

    stop(): void {
        for (const source of this.pending.splice(0)) {
            try {
                source.stop()
            } catch {
                // Already ended: nothing to stop.
            }
            source.disconnect()
        }
        this.nextStart = 0
    }

    async close(): Promise<void> {
        this.stop()
        if (this.context.state !== 'closed') await this.context.close()
    }
}
/** A player that keeps nothing and plays nothing, for the state-machine tests. */
export class SilentPlayer implements Player {
    queued = 0
    stopped = 0
    /** How many times the session let go of the player. Must be at most one per session. */
    closed = 0
    playing = false

    /** What a fake player reports as the wait before sound: a test sets it to check the sum. */
    audibleMs = 0

    enqueue(): Queued {
        this.queued += 1
        this.playing = true
        return { audibleInMs: this.audibleMs }
    }

    stop(): void {
        this.stopped += 1
        this.playing = false
    }

    async resume(): Promise<void> {}

    async close(): Promise<void> {
        this.closed += 1
        this.playing = false
    }
}
