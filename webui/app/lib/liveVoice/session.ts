/**
 * The client side of a live voice session (`specs/055-live-voice` FR-006, FR-019, FR-020, FR-042).
 *
 * A small state machine plus the loop that feeds it: chunks out, frames in, states rendered. All
 * the DOM lives in `capture.ts` and `player.ts`, and both are injected, so every transition here is
 * unit-testable without a browser.
 */

import {
    decodePcm16Base64,
    decodeServerFrame,
    interruptFrame,
    speechStartFrame,
    startFrame,
    stopFrame,
    utteranceEndFrame,
    audioFrame,
    encodePcm16Base64,
    type EndReason,
    type SessionState,
} from './frames'
import { EnergyVad, hangoverOf, rootMeanSquare } from './vad'
import type { Player } from './player'
import { liveVoiceMetricsEnabled, recordTurn } from './metrics'

/** What the UI renders. */
export interface LiveVoiceView {
    state: SessionState
    /** The FR-042 indicator: `waiting` is the agent thinking, never `reconnecting`. */
    thinking: boolean
    /** Milliseconds since the agent started thinking, for the elapsed counter. */
    thinkingMs: number
    /**
     * How long this session has been running, from `start()` to now. The row shows it as `m:ss`,
     * and until the field existed the row had nothing to show and printed `0:00` forever.
     */
    elapsedMs: number
    /** 0..1 input level, for the meter. */
    level: number
    /** The last transcript, so a person sees what was heard. */
    transcript: string
    /** A failure or ending reason to show until dismissed. */
    notice: string | null
    /** Whether the microphone is open, including an unfocused window (FR-038). */
    micOpen: boolean
}

/** What a session needs from the world; injected so the tests need no browser. */
/**
 * How long the microphone may take to open before the session gives up (N1).
 *
 * Generous on purpose: the delay is usually a person reading a permission prompt, and a shorter
 * deadline would kill sessions people meant to start. What it prevents is not slowness but the
 * absence of an exit — `connecting` used to have no other door than a capture that eventually
 * opened, while the server already held a session for it.
 */
export const CONNECTING_DEADLINE_MS = 30_000

export interface LiveVoiceDeps {
    /** Send one frame on the topic socket; `false` when the socket is not open. */
    send(frame: object): boolean
    player: Player
    /** Open the microphone; returns a handle whose `stop` releases it. */
    openCapture(options: {
        onChunk(samples: Int16Array): void
        onWindow(samples: Float32Array): void
        onLost(reason: string): void
    }): Promise<{ stop(): void; open: boolean }>
    /** Render. Called on every change. */
    onView(view: LiveVoiceView): void
    /** The recognition language, if the agent names one. */
    language?: string
    /**
     * How long a silence ends the utterance, from the agent's live voice settings (G1). The gate
     * runs here, in the client, so this is the only way the setting can mean anything.
     */
    endOfUtteranceMs?: number
    /** A clock, so the elapsed counter is testable. */
    now?(): number
    /** How long the device may take to open (N1). Absent leaves `CONNECTING_DEADLINE_MS`. */
    connectDeadlineMs?: number
}

/**
 * The state words, and which of them a session may move to next. Written down rather than implied:
 * a transition that is not here is a bug, not a shortcut.
 */
/**
 * How long a dropped session may stay `reconnecting` before it ends (FR-033).
 *
 * During the window the microphone stays open, because the session is still alive; past it the
 * session ends and everything is released, rather than hanging on a socket that never came back.
 */
export const RECONNECT_WINDOW_MS = 30_000

/**
 * Scales a window's RMS into the 0..1 the meter shows. A normal sentence at 16 kHz sits around
 * 0.1-0.15 RMS, so this puts it at 0.4-0.6 and leaves the top of the bar for a raised voice
 * instead of pinning it at 1 on every syllable.
 */
const LEVEL_GAIN = 4

/**
 * How long a person must actually be voiced before their silence is allowed to end the utterance
 * and interrupt the reply (FR-019, C8).
 *
 * The hangover alone cannot tell a sentence from a knock: the client sends the hangover's own
 * silence as audio, so a 20 ms blip arrives at the server as ~270 ms of PCM and passes
 * `MIN_UTTERANCE_MS` (`crates/vizier-core/src/live_voice/utterance.rs:10`). Measured on the voice
 * rather than on the buffer, a blip is 20 ms and a word is 300-plus.
 *
 * 300 ms: below the shortest word anyone says on purpose, above every blip a desk, a chair or an
 * AGC step produces. It only guards the *end* of an utterance — speech still opens one on the
 * first voiced window, so nothing a person says is ever unsent.
 */
const MIN_VOICED_MS = 300

const TRANSITIONS: Record<SessionState, SessionState[]> = {
    off: ['connecting'],
    connecting: ['listening', 'failed', 'ended'],
    listening: ['waiting', 'speaking', 'interrupted', 'reconnecting', 'ended', 'failed'],
    waiting: ['speaking', 'listening', 'interrupted', 'reconnecting', 'ended', 'failed'],
    speaking: ['listening', 'interrupted', 'waiting', 'reconnecting', 'ended', 'failed'],
    interrupted: ['listening', 'ended', 'failed', 'reconnecting'],
    reconnecting: ['listening', 'ended', 'failed'],
    ended: ['off', 'connecting'],
    failed: ['off', 'connecting'],
}

export class LiveVoiceClient {
    private state: SessionState = 'off'
    private view: LiveVoiceView = {
        state: 'off',
        thinking: false,
        thinkingMs: 0,
        elapsedMs: 0,
        level: 0,
        transcript: '',
        notice: null,
        micOpen: false,
    }
    private vad: EnergyVad
    private capture: { stop(): void; open: boolean } | null = null
    private utteranceId = 0
    private seq = 0
    private generation = 0
    /** When this turn was accepted: the elapsed counter starts here, per turn (FR-042). */
    private thinkingStartedAt = 0
    /** When the session started, for the clock the row shows. */
    private startedAt = 0
    /**
     * Whether a reply is audible. It raises the VAD's gate so the agent does not hear itself
     * (FR-026), and its end is what the end of the utterance silences (C8): the reply keeps
     * playing while the person speaks.
     */
    private replyAudible = false
    /**
     * The generation whose reply has been silenced on this side, or -1.
     *
     * Stopping the player is local and instant; the segments of the same reply are still in
     * flight and would be queued again a moment later, so the reply would come back to life
     * after being cut. Anything at or below this generation is refused on arrival. It is cleared
     * when a newer generation arrives or the socket reopens, so a reconnect can never leave the
     * session permanently mute.
     */
    private mutedGeneration = -1
    private tick: ReturnType<typeof setInterval> | null = null
    /** Whether this session has already let go of the microphone and the audio context. */
    private released = false
    /** The timer that gives up on a device that never opens (N1). */
    private connectDeadline: ReturnType<typeof setTimeout> | null = null
    /** When the socket dropped, or null while it is up. */
    private droppedAt: number | null = null
    /** The timer that ends a session whose socket never came back. */
    private reconnectTimer: ReturnType<typeof setTimeout> | null = null
    /**
     * The turn waiting for its first audible sample (T076). Set when the utterance closes and
     * cleared by the first segment that follows it, so a reply's second segment cannot overwrite
     * the sample its first one produced.
     */
    private pendingMetric: { endOfUtterance: number; silenceMs: number } | null = null

    constructor(private readonly deps: LiveVoiceDeps) {
        this.vad = new EnergyVad(3, hangoverOf(deps.endOfUtteranceMs))
    }

    get currentState(): SessionState {
        return this.state
    }

    /** The hangover this session's gate uses, in ms (G1). Read by the tests and by nobody else. */
    get hangoverMs(): number {
        return this.vad.hangover
    }

    get currentView(): LiveVoiceView {
        return this.view
    }

    /** Start a session: the one control, separate from the mic and the spoken-reply toggle. */
    async start(): Promise<void> {
        if (this.state !== 'off' && this.state !== 'ended' && this.state !== 'failed') return
        this.released = false
        this.startedAt = this.now()
        this.view.elapsedMs = 0
        // Nothing from the last visit is remembered (T063): an old reason, a stale transcript or a
        // leftover notice would greet the person as if it were news about this session. The
        // transcript was named here but not cleared, so a new call opened showing what the previous
        // one heard (F4).
        this.view.notice = null
        this.view.transcript = ''
        this.move('connecting')
        const sent = this.deps.send(startFrame(this.deps.language))
        if (!sent) {
            this.fail('connection_lost', 'the connection is not open')
            return
        }
        // N1: `startFrame` has already gone, so the server holds a session from here. If the device
        // never opens — a permission prompt left on screen, a browser that never answers — the
        // session used to sit in `connecting` for ever with no other door out.
        const deadlineMs = this.deps.connectDeadlineMs ?? CONNECTING_DEADLINE_MS
        this.connectDeadline = setTimeout(() => this.abandonConnect(deadlineMs), deadlineMs)
        try {
            const capture = await this.deps.openCapture({
                onChunk: (samples) => this.sendAudio(samples),
                onWindow: (samples) => this.onWindow(samples),
                onLost: (reason) => this.fail('permission_withdrawn', reason),
            })
            // The device arrived after the session was over — ended, failed, or released by a
            // page hide — so nothing is driving it and it must not stay open.
            //
            // Deliberately **not** "anything that is no longer `connecting`": the server publishes
            // `listening` as soon as it has accepted the start frame, which can arrive before the
            // device resolves. Treating that echo as "too late" closed the microphone of every
            // session where the provider was quicker than the permission prompt — `Listening` on
            // screen with `mic off`, no audio ever sent (N1 regression, `08cf067`).
            if (this.released || this.currentState === 'off' || this.currentState === 'ended' || this.currentState === 'failed') {
                capture.stop()
                return
            }
            this.clearConnectDeadline()
            this.capture = capture
            this.view.micOpen = this.capture.open
            await this.deps.player.resume()
            this.move('listening')
            this.tick = setInterval(() => this.render(), 100)
            this.render()
        } catch (e) {
            this.fail('no_device', (e as Error).message)
        }
    }

    /** The one interrupt control: stop the audio, tell the server, stay live (FR-020, SC-003). */
    interrupt(reason: 'button' | 'typed' = 'button'): void {
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        this.stopAudio()
        this.deps.send(interruptFrame(reason))
        if (this.state === 'speaking' || this.state === 'waiting') this.move('listening')
    }

    /** End the session and release the microphone. Safe to call twice. */
    stop(reason: 'pressed_again' | 'left_conversation' = 'pressed_again'): void {
        // One ending, one frame. A hidden page and then an unmount both call this (T090); the
        // second call must not tell the server to end a session it already ended.
        if (this.state === 'off' || this.released) return
        this.deps.send(stopFrame())
        this.release()
        this.move('ended', reason)
    }

    /** A frame arrived on the topic socket. */
    onFrame(raw: unknown): void {
        // Once the session has let go, nothing on the wire may reach it: a segment already in
        // flight would put audio in the queue of a closed player — reopening the audio context
        // `release()` closed — and a state word would write a defect notice onto a session that is
        // over. The row was hidden; it stays hidden until `start()`.
        if (this.released) return
        const frame = decodeServerFrame(raw)
        if (!frame) return
        switch (frame.type) {
            case 'state': {
                // A stale generation is a segment of a reply that was already interrupted.
                if (frame.generation && frame.generation < this.generation) return
                // `speaking` describes a reply that is being played. The generation we silenced is
                // not playing any more, so a `speaking` frame from it — still in the queue when the
                // person cut in — would put the word back with nothing behind it. The other words
                // from that same generation are the server's answer to the cut (`interrupted`,
                // `listening`) and are wanted, so only `speaking` is refused.
                if (
                    frame.state === 'speaking' &&
                    this.mutedGeneration >= 0 &&
                    frame.generation <= this.mutedGeneration
                ) {
                    return
                }
                const arrived = frame.generation || this.generation
                // A newer generation is a new reply: the silence that covered the old one no
                // longer applies to it.
                if (arrived > this.mutedGeneration) this.mutedGeneration = -1
                this.generation = arrived
                this.applyState(frame.state, frame.reason ?? undefined)
                break
            }
            case 'transcript':
                this.view.transcript = frame.text
                if (!frame.submitted && !frame.text) {
                    this.view.notice = 'that was too short to send'
                }
                this.render()
                break
            case 'error':
                this.view.notice = frame.message
                this.render()
                break
            case 'stall':
                // The socket fell behind and dropped audio: say it once, change no state.
                this.view.notice = `${frame.dropped} audio segments were dropped`
                this.render()
                break
            case 'speech': {
                // A segment of a reply that was already silenced: the queue would start it again
                // a moment after the person cut in.
                if (frame.generation <= this.mutedGeneration) return
                if (frame.generation < this.generation) return
                this.generation = frame.generation
                this.replyAudible = true
                const queued = this.deps.player.enqueue(
                    decodePcm16Base64(frame.base64),
                    frame.sample_rate,
                    frame.final
                )
                this.markFirstAudible(queued.audibleInMs)
                if (this.state !== 'speaking') this.move('speaking')
                if (frame.final) {
                    // The last segment only *starts* here; the player finishes it, and the session
                    // is listening again as soon as the queue drains.
                    setTimeout(() => {
                        this.replyAudible = false
                        if (this.state === 'speaking') this.move('listening')
                    }, 0)
                }
                break
            }
        }
    }

    /** The socket dropped: say reconnecting, stop the audio, keep the session (FR-033). */

    onSocketState(state: 'open' | 'connecting' | 'reconnecting' | 'offline'): void {
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        if (state === 'open') {
            if (this.state === 'reconnecting') {
                // The socket is back inside the window: nothing about this session is stale.
                this.clearReconnectDeadline()
                // A reconnect starts a fresh lane: nothing may stay muted from the old one.
                this.mutedGeneration = -1
                this.move('listening')
                // An idempotent start re-binds the server's lane to this new socket, which is what
                // keeps the session from going silent after a reconnect (T067). Nothing else is
                // re-sent: an utterance that was open when the socket dropped is dropped with it,
                // and a chunk or a `speech_start` from before the drop would be a duplicate the
                // server has no way to place.
                this.utteranceId = 0
                this.seq = 0
                this.deps.send(startFrame(this.deps.language))
            }
            return
        }
        if (state === 'reconnecting' || state === 'offline') {
            if (this.state === 'reconnecting') return
            this.stopAudio()
            this.move('reconnecting', 'connection_lost')
            // The microphone stays open for the window: the session is still alive, and a drop of a
            // second must not cost the person their device. FR-033 gives it thirty seconds; past
            // that the session ends and the microphone is released like any other ending.
            this.droppedAt = this.now()
            this.reconnectTimer = setTimeout(
                () => this.endIfStillDropped(),
                RECONNECT_WINDOW_MS
            )
        }
    }

    /**
     * End a session whose socket did not come back inside the window.
     *
     * `release()` is the same path every other ending takes, so the microphone and the audio
     * context are let go here exactly as they are on the End control — a session left
     * `reconnecting` forever would keep both alive with nothing to talk to.
     */
    private endIfStillDropped(): void {
        if (this.state !== 'reconnecting' || this.droppedAt === null) return
        if (this.now() - this.droppedAt < RECONNECT_WINDOW_MS) return
        this.clearReconnectDeadline()
        this.release()
        this.move('ended', 'connection_lost')
    }

    private clearReconnectDeadline(): void {
        this.droppedAt = null
        if (this.reconnectTimer) {
            clearTimeout(this.reconnectTimer)
            this.reconnectTimer = null
        }
    }

    /** For the tests, and for a tab whose timers were throttled while it was in the background. */
    checkReconnectDeadline(): void {
        this.endIfStillDropped()
    }

    /** The person typed a message: the audio stops, the session stays live (FR-025). */
    onTyped(): void {
        if (this.state === 'speaking' || this.state === 'waiting') this.interrupt('typed')
    }

    /**
     * The row's own transcript has reached the thread, so let the waveform have the middle slot
     * back (V9).
     *
     * `transcript` used to be cleared only by `start()`, so after the first word of a call the
     * meter never came back — for the rest of the session — and the row showed the previous turn's
     * sentence while a person asked themselves whether the microphone could still hear them. The
     * clear is **not** part of the state change: the words have to be adopted into the thread
     * first, or they disappear from the screen, which is what `LiveVoiceControl` does with them.
     */
    transcriptAdopted(): void {
        if (!this.view.transcript) return
        this.view.transcript = ''
        this.render()
    }

    // --- internals ---------------------------------------------------------

    private sendAudio(samples: Int16Array): void {
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        // No utterance open, no audio. The id belongs to the utterance the gate opened, and only
        // the gate opens one (`onWindow`). Inventing an id here is what broke the second
        // utterance of every call: the capture emits a chunk every 100 ms whether or not anyone
        // is talking (`capture.ts:56` then `:64`), so the first silent chunk claimed id 1, and
        // `!this.utteranceId` was never true again — `speech_start` went out at most once per
        // session and every later utterance was audio the server dropped without a word
        // (`push_audio` -> `None`, `session.rs:418-419`).
        if (!this.utteranceId) return
        this.deps.send(audioFrame(`u-${this.utteranceId}`, this.seq++, encodePcm16Base64(samples)))
    }

    private onWindow(samples: Float32Array): void {
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        // Read before `push`: the gate resets the utterance as part of reporting its end, so after
        // the call `voicedFor` is already zero and the guard below would reject everything.
        const voicedForMs = this.vad.voicedFor
        const decision = this.vad.push(samples, this.replyAudible)
        // The meter reads the audio itself, not the gate's verdict: a quiet window still has a
        // level, and the bar should fall on it rather than blink to nothing between syllables.
        // Smoothed 0.6/0.4 so it moves like a needle instead of strobing on every window.
        const rms = rootMeanSquare(samples)
        this.view.level = this.view.level * 0.6 + Math.min(1, rms * LEVEL_GAIN) * 0.4
        // The start of speech interrupts nothing (C8): while the person talks the reply keeps
        // playing, and the only thing a voiced window does is open the utterance the following
        // chunks belong to.
        if (!this.utteranceId && decision.voiced) {
            this.utteranceId += 1
            this.seq = 0
            this.deps.send(speechStartFrame(`u-${this.utteranceId}`))
        }
        if (decision.utteranceEnd) {
            // A blip is not a person finishing a sentence (bro-pm's DoD for the lost-TTS report):
            // a knock, a chair or an AGC step must not cut a reply that is being spoken. The
            // utterance the server already has is abandoned rather than closed — the next
            // `speech_start` replaces it, which `open_utterance` already does
            // (`session.rs:300-306`), so no new contract and no new frame.
            //
            // The cost is stated rather than hidden: an utterance shorter than `MIN_VOICED_MS`
            // leaves its audio on the server unclosed until the next one arrives. The server's
            // own cap (`max_utterance_seconds`) still bounds it, and a blip that never repeats
            // costs one abandoned buffer, not a lost sentence.
            if (this.replyAudible && voicedForMs < MIN_VOICED_MS) {
                if (this.utteranceId) {
                    this.utteranceId = 0
                    this.seq = 0
                }
                this.vad.resetUtterance()
                return
            }
            // The end of the utterance is the interruption, and it has to be felt on this tick:
            // the audio stops locally *before* the frame leaves, so the person hears silence
            // immediately rather than after a round trip (FR-019, SC-002). The server then
            // cancels the turn and answers with the state word.
            //
            // Both latency marks start here, and only here (T076): the utterance closing is the
            // event SC-001 and SC-002 time from, not the server's answer arriving. When the
            // recorder is off this is one boolean read and no allocation.
            const marking = liveVoiceMetricsEnabled()
            const endOfUtterance = marking ? this.now() : 0
            const stopping = this.now()
            this.stopAudio()
            if (marking) this.pendingMetric = { endOfUtterance, silenceMs: this.now() - stopping }
            if (this.utteranceId) this.deps.send(utteranceEndFrame(`u-${this.utteranceId}`))
            // The same tick: the person hears silence now, not after the server answers.
            if (this.state === 'speaking' || this.state === 'waiting') this.move('listening')
            this.utteranceId = 0
            this.seq = 0
            this.vad.resetUtterance()
        }
    }

    /**
     * The reply's first audible sample closes the turn's latency sample (T076, SC-001).
     *
     * The player says how long until this segment is heard; the wait since the utterance closed
     * comes from this client's clock. Adding them is the whole measurement — and the mark is
     * consumed, not read, so the reply's remaining segments cannot each overwrite it.
     */
    private markFirstAudible(audibleInMs: number): void {
        const pending = this.pendingMetric
        if (!pending) return
        this.pendingMetric = null
        recordTurn({
            firstAudibleMs: this.now() - pending.endOfUtterance + audibleInMs,
            silenceMs: pending.silenceMs,
        })
    }

    /**
     * Let go of everything a session holds: the microphone tracks, the audio context, the queued
     * segments, the render tick and the utterance in flight.
     *
     * Every terminal transition comes through here — the End control, leaving the conversation or
     * closing the page, a failure from either side, and an ending the server decides (idle, owner
     * gone). One path is the point: a second one would be a second chance to leave the microphone
     * open, and `AudioContext` keeps a device alive until it is closed.
     *
     * Idempotent: the first call does the work, and a later one costs nothing. `stop()` twice, an
     * ending frame after a local stop, and a failure racing a page close all land here exactly once.
     */
    private release(): void {
        if (this.released) return
        this.released = true
        // No reply is coming for the turn that was in flight: its mark would otherwise be spent on
        // the first segment of the next session.
        this.pendingMetric = null
        this.clearReconnectDeadline()
        this.clearConnectDeadline()
        this.capture?.stop()
        this.capture = null
        this.view.micOpen = false
        // The level is the microphone's, and the microphone is gone: `ended` must not leave the
        // bar standing at whatever the last window happened to be.
        this.view.level = 0
        // The clock stops where it stood rather than snapping back to zero (T062): the row's time
        // is the record of the session that just ran, and `0:00` next to `Ended` says the opposite
        // of what happened. `startedAt = 0` is what stops it advancing; `start()` zeroes the value
        // for the next session, so a frozen number can never leak forward.
        this.startedAt = 0
        this.stopAudio()
        // Closing is asynchronous and its outcome changes nothing the person can act on; the
        // player is finished with either way.
        void this.deps.player.close()
        if (this.tick) {
            clearInterval(this.tick)
            this.tick = null
        }
        this.vad.resetUtterance()
        this.utteranceId = 0
        this.seq = 0
    }

    /**
     * Give up on a device that never opened (N1): tell the server, then fail with a reason the
     * person can act on.
     *
     * The stop frame goes first, because from the server's side the session exists from the moment
     * `startFrame` left — and a client that never opens its microphone leaves a session nobody can
     * use and every other tab sees as taken.
     */
    /**
     * Give up on a device that never opened (N1).
     *
     * The question is whether a device ever arrived, **not** which word the row is showing. Gating
     * this on `state === 'connecting'` made the deadline die silently the moment anything moved the
     * session on — which is precisely the situation it exists for: a permission prompt nobody
     * answered while the server had already said `listening`. A session that is already over is
     * left alone, so an ending the person asked for is never repeated.
     */
    private abandonConnect(deadlineMs: number): void {
        this.connectDeadline = null
        if (this.capture !== null || this.released) return
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        this.deps.send(stopFrame())
        this.fail('no_device', `the microphone did not open within ${Math.round(deadlineMs / 1000)} seconds`)
    }

    private clearConnectDeadline(): void {
        if (!this.connectDeadline) return
        clearTimeout(this.connectDeadline)
        this.connectDeadline = null
    }

    private stopAudio(): void {
        this.deps.player.stop()
        this.replyAudible = false
        // Segments of this reply are still on their way; they must not restart it.
        this.mutedGeneration = this.generation
    }


    private applyState(next: SessionState, reason?: EndReason): void {
        // The server answers a start frame with `listening` as soon as it has accepted it, which
        // may be before the microphone has opened. That echo is not news: the only door into
        // `listening` is `start()`, where the device is actually attached. Without this, the echo
        // moved the session on and the device that arrived a moment later looked "too late" and
        // was closed — `Listening` with `mic off` and no audio (N1 regression, `08cf067`).
        if (next === 'listening' && this.state === 'connecting' && this.capture === null) {
            return
        }
        if (next === 'ended' || next === 'failed') {
            this.release()
            this.move(next, reason)
            return
        }
        if (next === 'reconnecting') this.stopAudio()
        if (next === 'interrupted') this.stopAudio()
        // `listening` from the server means the reply is over, whichever way it ended. `replyAudible`
        // was otherwise cleared only by a segment carrying `final` (`:323`) or by `stopAudio()` — and
        // an interrupted reply never sends that segment (`session.rs`: `final` is set only on the
        // last one, after the loop is allowed to finish). So after an interruption the gate stayed
        // raised at twice its factor, the person speaking softer than it was not heard, no utterance
        // end followed, and nothing cleared the flag: `listening` with no way back until the call was
        // ended. The server's own word is the signal that the reason is gone.
        if (next === 'listening') this.replyAudible = false
        this.move(next, reason)
    }

    private move(next: SessionState, reason?: EndReason): void {
        if (next === this.state) {
            this.render()
            return
        }
        const allowed = TRANSITIONS[this.state]
        if (!allowed.includes(next)) {
            // A transition the table does not allow is a defect: refuse it rather than pretend.
            this.view.notice = `ignored a move from ${this.state} to ${next}`
            this.render()
            return
        }
        this.state = next
        this.view.state = next
        // FR-042: `waiting` is the agent thinking, and it must never look like `reconnecting`.
        const thinking = next === 'waiting'
        if (thinking && !this.view.thinking) {
            // A new turn's silence is counted from its own start, so a second question never
            // shows the elapsed time of the first.
            this.thinkingStartedAt = this.now()
            this.view.thinkingMs = 0
        }
        if (!thinking) this.view.thinkingMs = 0
        this.view.thinking = thinking
        if (reason === 'connection_lost' && next === 'reconnecting') {
            this.view.notice = 'connection lost, reconnecting'
        }
        if ((next === 'ended' || next === 'failed') && reason) {
            this.view.notice = humanReason(reason)
        }
        this.render()
    }

    private fail(reason: EndReason, message: string): void {
        this.release()
        this.state = 'failed'
        this.view.state = 'failed'
        this.view.notice = message || humanReason(reason)
        this.render()
    }


    private render(): void {
        if (this.view.thinking) {
            this.view.thinkingMs = Math.max(0, this.now() - this.thinkingStartedAt)
        }
        if (this.startedAt > 0) this.view.elapsedMs = Math.max(0, this.now() - this.startedAt)
        this.deps.onView({ ...this.view })
        // A background tab has its timers throttled, so the deadline is also checked on the render
        // tick: a session must not stay `reconnecting` because a timeout was frozen.
        this.endIfStillDropped()
    }

    private now(): number {
        return this.deps.now ? this.deps.now() : performance.now()
    }
}

/** One wording per ending, so a person reads why rather than a code (SC-009). */
export function humanReason(reason: EndReason): string {
    switch (reason) {
        case 'pressed_again':
            return 'live voice is off'
        case 'left_conversation':
            return 'you left the conversation, so live voice ended'
        case 'page_closed':
            return 'the page was closed'
        case 'permission_withdrawn':
            return 'the microphone was taken away'
        case 'conversation_deleted':
            return 'this conversation was deleted'
        case 'agent_changed':
            return 'the agent was changed'
        case 'idle':
            return 'live voice ended after a while with nothing said'
        case 'connection_lost':
            return 'the connection dropped'
        case 'server_restart':
            return 'the server restarted'
        case 'no_device':
            return 'no microphone could be opened'
        case 'no_recognizer':
            return 'a recogniser is not configured for this agent'
        case 'no_voice':
            return 'a voice is not configured for this agent'
        case 'provider_failed':
            return 'the speech provider failed'
        case 'unintelligible':
            return 'that was not intelligible; say it again'
        case 'key_revoked':
            // T110: the credential behind this call was deleted. Its own sentence, because
            // "live voice is off" would hide the fact that something outside ended it.
            return 'the API key for this call was revoked'
        case 'key_expired':
            return 'the API key for this call expired'
        case 'moved':
            // T096: the person started a call somewhere else. One session is all they may
            // have, so this one ended — and the sentence has to say that, or it reads like
            // a failure they did not cause.
            return 'this call moved to another conversation'
        case 'unsupported_audio':
            // T102: the client asked for audio this release does not carry. It must not read
            // as a provider failure, or a person goes looking at credentials for a format bug.
            return 'this release carries 16 kHz mono audio'
    }
}
