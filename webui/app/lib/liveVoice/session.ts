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
import { EnergyVad } from './vad'
import type { Player } from './player'

/** What the UI renders. */
export interface LiveVoiceView {
    state: SessionState
    /** The FR-042 indicator: `waiting` is the agent thinking, never `reconnecting`. */
    thinking: boolean
    /** Milliseconds since the agent started thinking, for the elapsed counter. */
    thinkingMs: number
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
    /** A clock, so the elapsed counter is testable. */
    now?(): number
}

/**
 * The state words, and which of them a session may move to next. Written down rather than implied:
 * a transition that is not here is a bug, not a shortcut.
 */
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
        level: 0,
        transcript: '',
        notice: null,
        micOpen: false,
    }
    private vad = new EnergyVad()
    private capture: { stop(): void; open: boolean } | null = null
    private utteranceId = 0
    private seq = 0
    private generation = 0
    private startedAt = 0
    /** Whether a reply is audible: what raises the gate and arms the barge-in (FR-019, FR-026). */
    private replyAudible = false
    /** Set once a barge-in or button stop has silenced the current reply, so it is sent once. */
    private bargedIn = false
    private tick: ReturnType<typeof setInterval> | null = null

    constructor(private readonly deps: LiveVoiceDeps) {}

    get currentState(): SessionState {
        return this.state
    }

    get currentView(): LiveVoiceView {
        return this.view
    }

    /** Start a session: the one control, separate from the mic and the spoken-reply toggle. */
    async start(): Promise<void> {
        if (this.state !== 'off' && this.state !== 'ended' && this.state !== 'failed') return
        this.move('connecting')
        const sent = this.deps.send(startFrame(this.deps.language))
        if (!sent) {
            this.fail('connection_lost', 'the connection is not open')
            return
        }
        try {
            this.capture = await this.deps.openCapture({
                onChunk: (samples) => this.sendAudio(samples),
                onWindow: (samples) => this.onWindow(samples),
                onLost: (reason) => this.fail('permission_withdrawn', reason),
            })
            this.view.micOpen = this.capture.open
            await this.deps.player.resume()
            this.move('listening')
            this.startedAt = this.now()
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
        if (this.state === 'off') return
        this.deps.send(stopFrame())
        this.release()
        this.move('ended', reason)
    }

    /** A frame arrived on the topic socket. */
    onFrame(raw: unknown): void {
        const frame = decodeServerFrame(raw)
        if (!frame) return
        switch (frame.type) {
            case 'state': {
                // A stale generation is a segment of a reply that was already interrupted.
                if (frame.generation && frame.generation < this.generation) return
                this.generation = frame.generation || this.generation
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
                if (frame.generation < this.generation) return
                this.generation = frame.generation
                this.replyAudible = true
                this.bargedIn = false
                this.deps.player.enqueue(decodePcm16Base64(frame.base64), frame.sample_rate, frame.final)
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
                this.move('listening')
                // An idempotent start re-binds the server's lane to this new socket, which is what
                // keeps the session from going silent after a reconnect (T067).
                this.deps.send(startFrame(this.deps.language))
            }
            return
        }
        if (state === 'reconnecting' || state === 'offline') {
            this.stopAudio()
            this.move('reconnecting', 'connection_lost')
        }
    }

    /** The person typed a message: the audio stops, the session stays live (FR-025). */
    onTyped(): void {
        if (this.state === 'speaking' || this.state === 'waiting') this.interrupt('typed')
    }

    // --- internals ---------------------------------------------------------

    private sendAudio(samples: Int16Array): void {
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        if (!this.utteranceId) this.utteranceId += 1
        this.deps.send(audioFrame(`u-${this.utteranceId}`, this.seq++, encodePcm16Base64(samples)))
    }

    private onWindow(samples: Float32Array): void {
        if (this.state === 'off' || this.state === 'ended' || this.state === 'failed') return
        const decision = this.vad.push(samples, this.replyAudible)
        this.view.level = 0
        if (decision.bargeIn && this.replyAudible && !this.bargedIn) {
            // Barge-in: silence the audio on this tick, then tell the server (FR-019, SC-002).
            this.bargedIn = true
            this.stopAudio()
            this.deps.send(speechStartFrame(`u-${this.utteranceId + 1}`))
            this.move('listening')
            return
        }
        if (!this.utteranceId && decision.voiced) {
            // The person started speaking: open the utterance the next chunk belongs to.
            this.utteranceId += 1
            this.seq = 0
            this.deps.send(speechStartFrame(`u-${this.utteranceId}`))
        }
        if (decision.utteranceEnd) {
            if (this.utteranceId) this.deps.send(utteranceEndFrame(`u-${this.utteranceId}`))
            this.utteranceId = 0
            this.seq = 0
            this.vad.resetUtterance()
        }
    }

    private stopAudio(): void {
        this.deps.player.stop()
        this.replyAudible = false
    }

    private release(): void {
        this.capture?.stop()
        this.capture = null
        this.view.micOpen = false
        this.stopAudio()
        if (this.tick) {
            clearInterval(this.tick)
            this.tick = null
        }
        this.vad.resetUtterance()
        this.utteranceId = 0
        this.seq = 0
    }

    private applyState(next: SessionState, reason?: EndReason): void {
        if (next === 'ended' || next === 'failed') {
            this.release()
            this.move(next, reason)
            return
        }
        if (next === 'reconnecting') this.stopAudio()
        if (next === 'interrupted') this.stopAudio()
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
        if (thinking && !this.view.thinking) this.view.thinkingMs = 0
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
            this.view.thinkingMs = this.now() - (this.startedAt || this.now())
        }
        this.deps.onView({ ...this.view })
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
    }
}
