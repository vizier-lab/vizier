import { readFileSync } from 'node:fs'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { LiveVoiceClient, humanReason, type LiveVoiceDeps, type LiveVoiceView } from './session'
import { SilentPlayer } from './player'
import { stateWord } from '../../screens/chat/LiveVoiceBar'
import type { SessionState } from './frames'
import { liveVoiceReport, resetLiveVoiceMetrics, setLiveVoiceMetrics } from './metrics'

/** A client wired to a fake socket and a silent player, so every transition is observable. */
function harness(options: { captureFails?: string } = {}) {
    const sent: object[] = []
    const views: LiveVoiceView[] = []
    const player = new SilentPlayer()
    let clock = 1000
    const released = { count: 0 }
    const deps: LiveVoiceDeps = {
        send: (frame) => {
            sent.push(frame)
            return true
        },
        player,
        openCapture: async () => {
            if (options.captureFails) throw new Error(options.captureFails)
            return {
                stop: () => {
                    released.count += 1
                },
                open: true,
            }
        },
        onView: (view) => views.push(view),
        now: () => clock,
    }
    const client = new LiveVoiceClient(deps)
    return {
        client,
        sent,
        views,
        player,
        released,
        advance: (ms: number) => {
            clock += ms
        },
        lastView: () => views[views.length - 1],
        keys: () => sent.map((frame) => Object.keys(frame)[0]),
    }
}

describe('the live voice state machine', () => {
    it('walks off → connecting → listening → waiting → speaking → listening', async () => {
        const h = harness()
        await h.client.start()
        expect(h.client.currentState).toBe('listening')
        expect(h.keys()).toContain('live_voice_start')

        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        expect(h.client.currentState).toBe('waiting')
        expect(h.client.currentView.thinking).toBe(true)

        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 1,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        expect(h.client.currentState).toBe('speaking')

        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 1,
                seq: 1,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: true,
            },
        })
        await new Promise((resolve) => setTimeout(resolve, 0))
        expect(h.client.currentState).toBe('listening')
        expect(h.player.queued).toBe(2)
    })

    it('never sends start twice for one session', async () => {
        const h = harness()
        await h.client.start()
        await h.client.start()
        expect(h.keys().filter((key) => key === 'live_voice_start')).toHaveLength(1)
    })

    it('keeps thinking and reconnecting distinct (FR-042)', async () => {
        const h = harness()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        const thinking = h.lastView()
        expect(thinking.state).toBe('waiting')
        expect(thinking.thinking).toBe(true)
        expect(thinking.notice).toBeNull()

        h.client.onSocketState('reconnecting')
        const dropped = h.lastView()
        expect(dropped.state).toBe('reconnecting')
        expect(dropped.thinking).toBe(false)
        expect(dropped.notice).toBe('connection lost, reconnecting')
    })

    it('the button interrupt stops the audio and stays live', async () => {
        const h = harness()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        const before = h.player.stopped
        h.client.interrupt('button')
        expect(h.player.stopped).toBe(before + 1)
        expect(h.keys()).toContain('live_voice_interrupt')
        expect(h.client.currentState).toBe('listening')
    })

    // FR-025 is deferred (C7, spec.md) — this asserts a path the surface does not call;
    // re-enable with US3/T054.
    it.skip('a typed message interrupts with its own reason and is not spoken (FR-018 withdrawn)', async () => {
        const h = harness()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        h.client.onTyped()
        const frame = h.sent.find((f) => 'live_voice_interrupt' in f) as {
            live_voice_interrupt: { reason: string }
        }
        expect(frame.live_voice_interrupt.reason).toBe('typed')
        expect(h.keys()).not.toContain('live_voice_text')
    })

    it('drops a segment from a stale generation (FR-022)', async () => {
        const h = harness()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 5 } })
        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 4,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        expect(h.player.queued).toBe(0)
    })

    it('releases the microphone on stop and on a failure', async () => {
        const h = harness()
        await h.client.start()
        h.client.stop()
        expect(h.released.count).toBe(1)
        expect(h.client.currentState).toBe('ended')

        const failing = harness({ captureFails: 'no microphone was found' })
        await failing.client.start()
        expect(failing.client.currentState).toBe('failed')
        expect(failing.lastView().notice).toBe('no microphone was found')
    })

    it('refuses a move the table does not allow', () => {
        const h = harness()
        // Nothing has started, so there is no state to speak from.
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        expect(h.client.currentState).toBe('off')
    })
})

describe('the reason wording', () => {
    it('has one sentence per ending, none of them a code', () => {
        const reasons = [
            'pressed_again',
            'left_conversation',
            'page_closed',
            'permission_withdrawn',
            'conversation_deleted',
            'agent_changed',
            'idle',
            'connection_lost',
            'server_restart',
            'no_device',
            'no_recognizer',
            'no_voice',
            'provider_failed',
            'unintelligible',
            'unsupported_audio',
            'key_revoked',
            'key_expired',
            'moved',
        ] as const
        for (const reason of reasons) {
            const wording = humanReason(reason)
            expect(wording).not.toBe(reason)
            expect(wording.length).toBeGreaterThan(8)
        }
        // The pairs that must not share a wording.
        expect(humanReason('idle')).not.toBe(humanReason('connection_lost'))
        // T102: a wrong audio description is not a provider failure. If it read like one, a
        // person would go looking at credentials for a format bug.
        expect(humanReason('unsupported_audio')).not.toBe(humanReason('provider_failed'))
        expect(humanReason('unsupported_audio')).toContain('16 kHz')
        // T110: a revoked key and a lapsed one are two events, and neither may read like the
        // person switching the call off.
        expect(humanReason('key_revoked')).not.toBe(humanReason('key_expired'))
        expect(humanReason('key_revoked')).not.toBe(humanReason('pressed_again'))
        expect(humanReason('key_revoked')).toContain('revoked')
        expect(humanReason('key_expired')).toContain('expired')
        // T096: a moved call is not a failure and not a switch-off.
        expect(humanReason('moved')).not.toBe(humanReason('pressed_again'))
        expect(humanReason('moved')).toContain('another conversation')
    })
})


/** The composer's own harness, with the capture callbacks exposed so a test can drive audio. */
function driven(options: { sendReturns?: (frame: object) => boolean } = {}) {
    const sent: object[] = []
    const views: LiveVoiceView[] = []
    const player = new SilentPlayer()
    let clock = 1000
    let capture: {
        onChunk(samples: Int16Array): void
        onWindow(samples: Float32Array): void
        onLost(reason: string): void
    } | null = null
    /** How many times the session released the microphone; more than one is a leak. */
    let microphones = 0
    const client = new LiveVoiceClient({
        send: (frame) => {
            sent.push(frame)
            return options.sendReturns ? options.sendReturns(frame) : true
        },
        player,
        openCapture: async (callbacks) => {
            capture = callbacks
            return {
                stop: () => {
                    microphones += 1
                },
                open: true,
            }
        },
        onView: (view) => views.push(view),
        now: () => clock,
    })
    return {
        client,
        sent,
        player,
        views,
        microphones: () => microphones,
        /** The device is taken away mid-session, which is a local failure. */
        lost: (reason: string) => capture?.onLost(reason),
        last: () => views[views.length - 1],
        advance: (ms: number) => {
            clock += ms
        },
        /** One 20 ms window of voice, which is what the gate decides on. `level` is its RMS. */
        speak: (ms = 200, level = 0.2) => {
            const window = new Float32Array(320)
            for (let i = 0; i < window.length; i++) window[i] = i % 2 === 0 ? level : -level
            for (let elapsed = 0; elapsed < ms; elapsed += 20) capture?.onWindow(window)
        },
        /** Silence for the hangover, which ends the utterance. */
        stopSpeaking: () => {
            const silent = new Float32Array(320)
            for (let elapsed = 0; elapsed < 900; elapsed += 20) capture?.onWindow(silent)
        },
    }
}

describe('the raised gate is released when the server says the reply is over', () => {
    it('a reply that ends without a final segment cannot leave the person unheard', async () => {
        // The failure this closes: `replyAudible` was cleared only by a segment carrying `final`
        // (`session.ts:323`) or by `stopAudio()`. An interrupted reply never sends that segment, so
        // the flag stayed true, the gate stayed at twice its factor, and the person — quieter than
        // the raised gate but louder than the real one — was never heard again. No utterance end
        // followed, so nothing ever cleared the flag: `listening` with no way back until `End`.
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 1,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        // 0.025 RMS clears the quiet gate (floor x 3) and falls under the raised one (x 6), so the
        // same voice is the agent's own echo over the speakers and the person in silence.
        h.speak(40, 0.025)
        expect(
            h.sent.filter((frame) => 'live_voice_speech_start' in frame),
            'while the reply is audible this is the response, not the person'
        ).toHaveLength(0)

        // The reply is over — and no `final` segment ever arrived to say so.
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })

        // The same voice now opens an utterance, because the gate came back down to its real factor.
        h.speak(40, 0.025)
        expect(
            h.sent.filter((frame) => 'live_voice_speech_start' in frame),
            'the server said listening, so the reply is not audible any more'
        ).toHaveLength(1)
    })
})

describe('FR-042 — the thinking indicator', () => {
    it('says thinking on waiting, and never in the reconnecting wording', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        expect(h.last().thinking).toBe(true)
        expect(stateWord(h.last())).toBe('agent sedang berpikir')

        h.client.onSocketState('reconnecting')
        expect(h.last().thinking).toBe(false)
        expect(stateWord(h.last())).toBe('reconnecting')
        // The two words are never the same string, which is what keeps them apart on screen.
        expect(stateWord({ ...h.last(), state: 'waiting' })).not.toBe(stateWord({ ...h.last(), state: 'reconnecting' }))
    })

    it('the elapsed time runs while the agent thinks, and starts from zero each turn', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        h.advance(3000)
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        expect(h.last().thinkingMs).toBeGreaterThanOrEqual(3000)

        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 2 } })
        expect(h.last().thinkingMs).toBe(0)
    })

    it('clears on the first audible segment', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        expect(h.last().thinking).toBe(true)
        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 1,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        expect(h.last().thinking).toBe(false)
        expect(h.last().state).toBe('speaking')
    })

    it('leaves nothing lit when an interruption or an ending follows', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        h.client.interrupt('button')
        expect(h.last().thinking).toBe(false)
        expect(h.last().state).toBe('listening')

        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 2 } })
        h.client.onFrame({
            live_voice: { type: 'state', state: 'ended', generation: 2, reason: 'idle' },
        })
        expect(h.last().thinking).toBe(false)
        expect(h.last().state).toBe('ended')
    })

    it('is off on a failure reached from thinking', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'waiting', generation: 1 } })
        h.client.onFrame({
            live_voice: { type: 'state', state: 'failed', generation: 1, reason: 'provider_failed' },
        })
        expect(h.last().thinking).toBe(false)
        expect(h.last().state).toBe('failed')
    })

    it('every state word is its own word', () => {
        const words = (
            [
                'off',
                'connecting',
                'listening',
                'waiting',
                'speaking',
                'interrupted',
                'reconnecting',
                'ended',
                'failed',
            ] as const
        ).map((state) => stateWord({ ...h0(), state }))
        expect(new Set(words).size).toBe(words.length)
    })
})

function h0(): LiveVoiceView {
    return {
        state: 'off',
        thinking: false,
        thinkingMs: 0,
        elapsedMs: 0,
        level: 0,
        transcript: '',
        notice: null,
        micOpen: false,
    }
}

describe('T042 — a refused frame is not an ending', () => {
    it('keeps the session alive when the socket refuses audio but is still open', async () => {
        // `ClientConnection::send` says `false` for a full queue and a closed socket alike; only
        // the second ends a session (D10). Here every audio frame is refused, the socket is open.
        const h = driven({
            sendReturns: (frame) => !('live_voice_audio' in frame) && !('live_voice_speech_start' in frame),
        })
        await h.client.start()
        expect(h.client.currentState).toBe('listening')

        h.speak()
        h.stopSpeaking()

        expect(h.client.currentState).toBe('listening')
        expect(h.last().notice).toBeNull()
        expect(h.last().state).not.toBe('failed')
        expect(h.last().state).not.toBe('ended')
    })

    it('never turns a dropped audio frame into a stall notice on the client', async () => {
        const h = driven({ sendReturns: (frame) => !('live_voice_audio' in frame) })
        await h.client.start()
        h.speak()
        h.stopSpeaking()
        // The stall is reported by the socket's own loop, once per episode, not invented here.
        expect(h.last().notice).toBeNull()
        expect(h.sent.some((frame) => JSON.stringify(frame).includes('stall'))).toBe(false)
    })

    it('end, and only end, is what a closed socket produces', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({
            live_voice: { type: 'state', state: 'ended', generation: 1, reason: 'page_closed' },
        })
        expect(h.client.currentState).toBe('ended')
        expect(h.last().state).toBe('ended')
    })
})
/** The order of the two things T045 is about: the audio stopping, and the frame leaving. */
function ordered(options: { sendReturns?: (frame: object) => boolean } = {}) {
    const log: string[] = []
    const sent: object[] = []
    const views: LiveVoiceView[] = []
    let capture: {
        onChunk(samples: Int16Array): void
        onWindow(samples: Float32Array): void
        onLost(reason: string): void
    } | null = null
    const player = new SilentPlayer()
    const stop = player.stop.bind(player)
    player.stop = () => {
        stop()
        log.push('stop')
    }
    const client = new LiveVoiceClient({
        send: (frame) => {
            sent.push(frame)
            const key = Object.keys(frame)[0] ?? 'unknown'
            log.push(key)
            return options.sendReturns ? options.sendReturns(frame) : true
        },
        player,
        openCapture: async (callbacks) => {
            capture = callbacks
            return { stop: () => {}, open: true }
        },
        onView: (view) => views.push(view),
        now: () => 1000,
    })
    const voice = (ms: number) => {
        const window = new Float32Array(320)
        for (let i = 0; i < window.length; i++) window[i] = i % 2 === 0 ? 0.2 : -0.2
        for (let elapsed = 0; elapsed < ms; elapsed += 20) capture?.onWindow(window)
    }
    const silence = (ms: number) => {
        const window = new Float32Array(320)
        for (let elapsed = 0; elapsed < ms; elapsed += 20) capture?.onWindow(window)
    }
    /** One 100 ms chunk of audio, as the capture emits alongside the analysis windows. */
    const chunk = () => capture?.onChunk(new Int16Array(1600))
    return { client, log, sent, player, voice, silence, chunk, views }
}

/** One segment, so the session believes a reply is audible. */
const audible = {
    live_voice: {
        type: 'speech' as const,
        generation: 1,
        seq: 0,
        sample_rate: 16000,
        format: 'pcm16',
        base64: 'AAECAw==',
        final: false,
    },
}

describe('T045 — the interruption lands at the end of the utterance (C8)', () => {
    it('keeps the reply playing while the person speaks', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible)
        expect(h.client.currentState).toBe('speaking')

        const before = h.player.stopped
        h.voice(400)
        h.chunk()
        // The reply is still audible: nothing stopped it, and no interruption was announced.
        expect(h.player.stopped, 'the agent keeps talking while the person talks').toBe(before)
        expect(h.log).not.toContain('live_voice_interrupt')
        expect(h.client.currentState).toBe('speaking')
        // The words are already being captured, which is what makes the later cut possible.
        expect(h.log).toContain('live_voice_speech_start')
        expect(h.log).toContain('live_voice_audio')
    })

    it('stops the audio at the end of the utterance, then sends the frame in that tick', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible)
        h.voice(400)
        h.log.length = 0

        h.silence(900)

        const stopAt = h.log.indexOf('stop')
        const endAt = h.log.indexOf('live_voice_utterance_end')
        expect(stopAt, 'the audio stops when the person finishes').toBeGreaterThanOrEqual(0)
        expect(endAt, 'the utterance end is sent on that tick').toBeGreaterThanOrEqual(0)
        expect(stopAt, 'silence is felt before the round trip, not after').toBeLessThan(endAt)
        // Nothing else was said on that tick: no interrupt frame, no second speech_start.
        expect(h.log).not.toContain('live_voice_interrupt')
        expect(h.player.playing).toBe(false)
    })

    it('sends speech_start once per utterance, and utterance_end once', async () => {
        const h = ordered()
        await h.client.start()
        h.voice(400)
        h.silence(900)
        h.voice(400)
        h.silence(900)
        expect(h.log.filter((entry) => entry === 'live_voice_speech_start')).toHaveLength(2)
        expect(h.log.filter((entry) => entry === 'live_voice_utterance_end')).toHaveLength(2)
    })

    it('a chunk during silence does not swallow the next speech_start', async () => {
        // What a real browser does and this suite never did: the capture emits one 100 ms chunk
        // every 100 ms, whether or not anyone is talking (`capture.ts:56` window, then `:64`
        // chunk). `sendAudio` used to bump `utteranceId` for that chunk, and `onWindow` opens an
        // utterance only when `!utteranceId` — so after the first silent chunk the id was never
        // zero again, `speech_start` went out at most once per session, and every later utterance
        // was audio the server dropped without a word (`push_audio` -> `None`). That is the
        // report: "I speak again and only `listening` happens".
        const h = ordered()
        await h.client.start()

        // Silence, with the chunks that flow anyway.
        h.chunk()
        h.chunk()

        // Then a person speaks, finishes, and speaks again.
        h.voice(400)
        h.silence(900)
        h.chunk()
        h.voice(400)
        h.silence(900)

        expect(
            h.log.filter((entry) => entry === 'live_voice_speech_start'),
            'two utterances spoken, so two utterances opened'
        ).toHaveLength(2)
        expect(
            h.log.filter((entry) => entry === 'live_voice_utterance_end'),
            'and both closed'
        ).toHaveLength(2)
    })

    it('a 100 ms burst over a live reply is a blip, not an ending (superseded expectation)', async () => {
        // This test used to assert the opposite, and the change is deliberate: a 100 ms burst
        // closed the utterance, stopped the audio locally and cancelled the turn, so a knock, a
        // chair or an AGC step could cut an answer that was being spoken (bro-pm's DoD for the
        // lost-TTS report). It now has to be **voice** of `MIN_VOICED_MS`, not a buffer length,
        // before silence may end an utterance at all.
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible)
        h.voice(100)
        h.silence(900)
        expect(h.log, 'the reply keeps playing: nobody interrupted it').not.toContain(
            'live_voice_utterance_end'
        )
    })

    it('with no reply playing, the same burst still ends the utterance for the server to judge', async () => {
        // FR-024 is untouched: when nothing is being spoken over, the client still closes the
        // utterance and the server still decides that it was too short to be a word.
        const h = ordered()
        await h.client.start()
        h.voice(100)
        h.silence(900)
        expect(h.log).toContain('live_voice_utterance_end')
    })

    it('the frame order is unchanged: chunks, then the end', async () => {
        const h = ordered()
        await h.client.start()
        h.voice(400)
        h.chunk()
        h.chunk()
        h.silence(900)
        const kinds = h.log.filter((entry) => entry.startsWith('live_voice_'))
        const lastAudio = kinds.lastIndexOf('live_voice_audio')
        const end = kinds.indexOf('live_voice_utterance_end')
        expect(lastAudio).toBeGreaterThanOrEqual(0)
        expect(end).toBeGreaterThan(lastAudio)
    })
})

describe('T045 — a silenced reply cannot come back to life', () => {
    it('refuses a late segment of the generation it just cut, and opens for the next one', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible) // generation 1 is audible
        expect(h.client.currentState).toBe('speaking')
        const queuedBefore = h.player.queued

        // The person speaks, then finishes: the audio stops on that tick.
        h.voice(400)
        h.chunk()
        h.silence(900)
        expect(h.player.stopped).toBeGreaterThan(0)

        // Segments of that same reply are still in flight. Arriving now, they must not restart it.
        h.client.onFrame(audible)
        h.client.onFrame({ live_voice: { ...audible.live_voice, seq: 1 } })
        expect(h.player.queued, 'a cut reply stays cut').toBe(queuedBefore)
        expect(h.player.playing, 'nothing is playing again').toBe(false)
        expect(h.client.currentState).not.toBe('speaking')

        // A newer generation is a new reply: the silence does not carry over to it.
        h.client.onFrame({
            live_voice: { type: 'state', state: 'listening', generation: 2 },
        })
        h.client.onFrame({ live_voice: { ...audible.live_voice, generation: 2 } })
        expect(h.player.playing, 'the next reply is heard').toBe(true)
        expect(h.client.currentState).toBe('speaking')
    })

    it('a reconnect never leaves the session muted', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible)
        h.voice(400)
        h.silence(900)
        expect(h.player.playing).toBe(false)

        h.client.onSocketState('reconnecting')
        h.client.onSocketState('open')
        // Same generation as the one that was muted, and it is heard: the lane is new, and a
        // reconnect that stayed silent would look like a dead session.
        h.client.onFrame(audible)
        expect(h.player.playing, 'a reconnected session still speaks').toBe(true)
    })
})

describe('T045 — the word follows the audio', () => {
    it('refuses a stale speaking word after the cut, and keeps the answer to it', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible) // generation 1 audible
        h.voice(400)
        h.chunk()
        h.silence(900)
        // The cut happened: audio stopped and the session is listening again.
        expect(h.player.stopped).toBeGreaterThan(0)
        expect(h.client.currentState).toBe('listening')

        // A `speaking` frame of the generation we silenced is still in the queue. Taking it would
        // say the agent is talking with no audio behind it.
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        expect(h.client.currentState, 'a cut reply does not claim to be speaking').toBe('listening')

        // The same generation's real answer — the interruption — is wanted.
        h.client.onFrame({ live_voice: { type: 'state', state: 'interrupted', generation: 1 } })
        expect(h.client.currentState).toBe('interrupted')
    })

    it('accepts speaking again once a newer generation arrives', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible)
        h.voice(400)
        h.chunk()
        h.silence(900)
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 2 } })
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 2 } })
        expect(h.client.currentState, 'the next reply is allowed to say it is speaking').toBe(
            'speaking'
        )
    })
})

describe('C2 — every ending lets go of the microphone and the audio context', () => {
    /** A session that has started and is listening, with one segment playing. */
    async function live() {
        const h = driven()
        await h.client.start()
        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 1,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        expect(h.client.currentState).toBe('speaking')
        return h
    }

    it('the End control releases once and closes the context', async () => {
        const h = await live()
        h.client.stop('pressed_again')
        expect(h.client.currentState).toBe('ended')
        expect(h.microphones(), 'the tracks are stopped exactly once').toBe(1)
        expect(h.player.closed, 'the audio context is closed, not left running').toBe(1)
        expect(h.player.playing).toBe(false)
        expect(h.last().micOpen).toBe(false)
        // The control is offered again from an ending, and a second stop is free.
        h.client.stop('pressed_again')
        expect(h.microphones()).toBe(1)
        expect(h.player.closed).toBe(1)
    })

    it('leaving the conversation releases the same way', async () => {
        const h = await live()
        h.client.stop('left_conversation')
        expect(h.client.currentState).toBe('ended')
        expect(h.last().notice).toBe('you left the conversation, so live voice ended')
        expect(h.microphones()).toBe(1)
        expect(h.player.closed).toBe(1)
        expect(h.last().micOpen).toBe(false)
    })

    it('a local failure releases too — the microphone is never left open by an error', async () => {
        const h = await live()
        // The device is taken away while the reply plays: `fail()` is the local path, not a frame.
        h.lost('the microphone was taken away')
        expect(h.client.currentState).toBe('failed')
        expect(h.last().notice).toBe('the microphone was taken away')
        expect(h.microphones(), 'the tracks are stopped once').toBe(1)
        expect(h.player.closed, 'the context is closed').toBe(1)
        expect(h.last().micOpen).toBe(false)
    })

    it('a failure the server reports releases the same way', async () => {
        const h = await live()
        h.client.onFrame({
            live_voice: { type: 'state', state: 'failed', generation: 1, reason: 'provider_failed' },
        })
        expect(h.client.currentState).toBe('failed')
        expect(h.microphones()).toBe(1)
        expect(h.player.closed).toBe(1)
        expect(h.last().micOpen).toBe(false)
    })
    it('an ending the server decides releases the same way', async () => {
        const h = await live()
        h.client.onFrame({
            live_voice: { type: 'state', state: 'ended', generation: 1, reason: 'idle' },
        })
        expect(h.client.currentState).toBe('ended')
        expect(h.last().notice).toBe('live voice ended after a while with nothing said')
        expect(h.microphones()).toBe(1)
        expect(h.player.closed).toBe(1)
        expect(h.last().micOpen).toBe(false)
    })

    it('an ending frame after a local stop does not release twice', async () => {
        const h = await live()
        h.client.stop('pressed_again')
        h.client.onFrame({
            live_voice: { type: 'state', state: 'ended', generation: 1, reason: 'idle' },
        })
        expect(h.microphones(), 'two endings, one release').toBe(1)
        expect(h.player.closed).toBe(1)
    })

    it('the tick stops with the session, so nothing keeps rendering after it ends', async () => {
        const h = await live()
        const before = h.views.length
        h.client.stop('pressed_again')
        const afterStop = h.views.length
        // The 100 ms tick would add a view on every interval; none arrives after the release.
        await new Promise((resolve) => setTimeout(resolve, 250))
        expect(h.views.length).toBe(afterStop)
        expect(afterStop).toBeGreaterThan(before)
    })
})

describe('C2 — a session that ends can be started again', () => {
    it('starts a second session after an ending, with a fresh microphone and player', async () => {
        const h = driven()
        await h.client.start()
        expect(h.client.currentState).toBe('listening')
        h.client.stop('pressed_again')
        expect(h.player.closed).toBe(1)

        await h.client.start()
        expect(h.client.currentState).toBe('listening')
        expect(h.microphones(), 'the first session released its tracks').toBe(1)
        expect(h.last().micOpen).toBe(true)

        h.client.stop('pressed_again')
        expect(h.microphones(), 'and the second released its own').toBe(2)
        expect(h.player.closed).toBe(2)
    })
})

describe('C3 — a dropped session ends at thirty seconds, not never (T067)', () => {
    /** A live session with one utterance already open and captured. */
    async function live() {
        const h = driven()
        await h.client.start()
        expect(h.client.currentState).toBe('listening')
        h.speak(200)
        h.advance(200)
        return h
    }

    it('a drop keeps the microphone open for the window and stops the audio', async () => {
        const h = await live()
        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 1,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        h.client.onSocketState('reconnecting')

        expect(h.client.currentState).toBe('reconnecting')
        expect(h.last().notice).toBe('connection lost, reconnecting')
        expect(h.player.playing).toBe(false)
        // Still alive: the device is not taken away because a socket hiccuped.
        expect(h.last().micOpen).toBe(true)
        expect(h.microphones(), 'the window has not closed yet').toBe(0)
        expect(h.player.closed).toBe(0)
    })

    it('a socket back at 29 s resumes the same session and re-binds it exactly once', async () => {
        const here = await live()
        const before = here.sent.filter(
            (frame) => 'live_voice_start' in (frame as Record<string, unknown>)
        ).length
        const utteranceAtDrop = here.sent.length

        here.client.onSocketState('reconnecting')
        here.advance(29_000)
        here.client.checkReconnectDeadline()
        expect(here.client.currentState, 'inside the window the session is still alive').toBe(
            'reconnecting'
        )

        here.client.onSocketState('open')
        expect(here.client.currentState).toBe('listening')
        const after = here.sent.filter(
            (frame) => 'live_voice_start' in (frame as Record<string, unknown>)
        ).length
        expect(after - before, 'the lane is re-bound once, not twice').toBe(1)
        expect(here.microphones(), 'the device was never taken away').toBe(0)

        // Nothing from before the drop is re-sent: no chunk, no speech_start, no utterance_end.
        const resent = here.sent.slice(utteranceAtDrop + 1)
        expect(
            resent.some(
                (frame) =>
                    'live_voice_audio' in (frame as Record<string, unknown>) ||
                    'live_voice_speech_start' in (frame as Record<string, unknown>) ||
                    'live_voice_utterance_end' in (frame as Record<string, unknown>)
            ),
            'a reopened socket replays nothing'
        ).toBe(false)
    })

    it('past thirty seconds the session ends and lets go of everything exactly once', async () => {
        const h = await live()
        h.client.onSocketState('reconnecting')
        h.advance(31_000)
        // The render tick drives the deadline, so a throttled timeout cannot leave it hanging.
        await new Promise((resolve) => setTimeout(resolve, 220))

        expect(h.client.currentState).toBe('ended')
        expect(h.last().notice).toBe('the connection dropped')
        expect(h.last().micOpen).toBe(false)
        expect(h.microphones(), 'the device is released once').toBe(1)
        expect(h.player.closed).toBe(1)
    })

    it('an ending is not sent on a socket that is gone', async () => {
        const h = await live()
        const before = h.sent.length
        h.client.onSocketState('offline')
        h.advance(31_000)
        await new Promise((resolve) => setTimeout(resolve, 220))
        expect(h.client.currentState).toBe('ended')
        const sentSince = h.sent.slice(before)
        expect(
            sentSince.some((frame) => 'live_voice_stop' in (frame as Record<string, unknown>)),
            'a closed socket is not asked to carry a goodbye'
        ).toBe(false)
    })

    it('a second drop after a resume starts a fresh window', async () => {
        const h = await live()
        h.client.onSocketState('reconnecting')
        h.advance(10_000)
        h.client.onSocketState('open')
        h.client.onSocketState('reconnecting')
        h.advance(25_000)
        h.client.checkReconnectDeadline()
        // Twenty-five seconds into the *second* window: not expired.
        expect(h.client.currentState).toBe('reconnecting')
        h.advance(6_000)
        h.client.checkReconnectDeadline()
        expect(h.client.currentState).toBe('ended')
    })
})

describe('the session clock', () => {
    it('counts from start, and freezes at what it counted when the session ends (T062)', async () => {
        const h = driven()
        await h.client.start()
        expect(h.client.currentView.elapsedMs).toBe(0)

        h.advance(3000)
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        expect(h.client.currentView.elapsedMs).toBeGreaterThanOrEqual(3000)

        h.client.stop('pressed_again')
        expect(h.client.currentState).toBe('ended')
        // Changed with T062: the number is a record of the session that ran, not a reset. What
        // matters is that it stops moving, and the test below shows it does.
        expect(h.client.currentView.elapsedMs).toBeGreaterThanOrEqual(3000)
    })

    it('starts again from zero on the next session', async () => {
        const h = driven()
        await h.client.start()
        h.advance(5000)
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        expect(h.client.currentView.elapsedMs).toBeGreaterThanOrEqual(5000)
        h.client.stop('pressed_again')

        await h.client.start()
        expect(h.client.currentView.elapsedMs).toBe(0)
    })
})

/**
 * T090 — the row is hidden by several means, and every one of them has to end the session once.
 *
 * The client-side legs (an End press, a failure, an ending the server decides, the tick stopping)
 * are already pinned in the C2 block above. What is added here is what that block never asked:
 * the *second* call that an unmount after a page hide produces, and the frames still in flight
 * when the row goes away.
 */
describe('T090 — a hidden row ends the session once, and stays ended', () => {
    it('a page hide and then an unmount send one stop frame, not two', async () => {
        const h = driven()
        await h.client.start()
        // `LiveVoiceControl` calls the same thing from `pagehide` and from its unmount cleanup.
        h.client.stop('left_conversation')
        h.client.stop('left_conversation')
        expect(h.sent.filter((frame) => 'live_voice_stop' in frame), 'one ending, one frame').toHaveLength(1)
        expect(h.microphones()).toBe(1)
        expect(h.player.closed).toBe(1)
        expect(h.last().micOpen).toBe(false)
    })

    it('a segment still on the wire cannot reach the player after the row is gone', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 3 } })
        h.client.stop('left_conversation')
        const queued = h.player.queued

        // A newer generation, so the mute from the cut does not explain the silence: this is the
        // `released` gate, not the barge-in one.
        h.client.onFrame({
            live_voice: {
                type: 'speech',
                generation: 4,
                seq: 0,
                sample_rate: 16000,
                format: 'pcm16',
                base64: 'AAECAw==',
                final: false,
            },
        })
        expect(h.player.queued, 'nothing is queued into a closed player').toBe(queued)
        expect(h.player.closed).toBe(1)
        expect(h.client.currentState).toBe('ended')
    })

    it('a state word after the row is gone writes no defect notice onto an ended session', async () => {
        const h = driven()
        await h.client.start()
        h.client.stop('left_conversation')
        const notice = h.last().notice

        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 4 } })
        expect(h.last().notice).toBe(notice)
        expect(h.last().notice).not.toMatch(/ignored a move/)
        expect(h.client.currentState).toBe('ended')
    })

    it('and a session can still be started after all of it', async () => {
        const h = driven()
        await h.client.start()
        h.client.stop('left_conversation')
        h.client.stop('left_conversation')
        await h.client.start()
        expect(h.client.currentState).toBe('listening')
        expect(h.last().micOpen).toBe(true)
        expect(h.microphones(), 'the old microphone was released, the new one is open').toBe(1)
    })
})

/**
 * T076 — the latency marks. Two timestamps taken at two different events, and the test says so by
 * moving the clock between them: a single number read twice would produce 0, and a mark taken at
 * the wrong event would produce 300 without the player's share.
 */
describe('T076 — the latency marks are two events', () => {
    beforeEach(() => {
        resetLiveVoiceMetrics()
        setLiveVoiceMetrics(true)
    })

    afterEach(() => {
        setLiveVoiceMetrics(false)
        resetLiveVoiceMetrics()
        vi.restoreAllMocks()
    })

    /** The reply's first segment, as the socket delivers it. */
    const segment = (generation = 1, seq = 0) => ({
        live_voice: {
            type: 'speech' as const,
            generation,
            seq,
            sample_rate: 16000,
            format: 'pcm16' as const,
            base64: 'AAECAw==',
            final: false,
        },
    })

    it('counts from the utterance closing to the first audible sample, not to the frame', async () => {
        vi.spyOn(console, 'info').mockImplementation(() => {})
        const h = driven()
        await h.client.start()
        h.player.audibleMs = 200

        // The 800 ms hangover fires at clock 1000: that is the zero.
        h.speak(200)
        h.stopSpeaking()
        // The reply's first segment arrives 300 ms later and will be heard 200 ms after that.
        h.advance(300)
        h.client.onFrame(segment())

        const report = liveVoiceReport()
        expect(report.n).toBe(1)
        expect(report.firstAudible.p50).toBe(500)
        // Not 0 (one event read twice) and not 300 (the frame's arrival, the player's share lost).
        expect(report.firstAudible.p50).not.toBe(0)
        expect(report.firstAudible.p50).not.toBe(300)
    })

    it('one turn, one sample: the reply\'s second segment does not overwrite it', async () => {
        vi.spyOn(console, 'info').mockImplementation(() => {})
        const h = driven()
        await h.client.start()
        h.player.audibleMs = 200
        h.speak(200)
        h.stopSpeaking()
        h.advance(300)
        h.client.onFrame(segment(1, 0))
        expect(liveVoiceReport().firstAudible.p50).toBe(500)

        // A later segment of the same reply would measure 900 + 200 = 1100 if it were allowed.
        h.advance(600)
        h.client.onFrame(segment(1, 1))
        expect(liveVoiceReport().n, 'still one turn').toBe(1)
        expect(liveVoiceReport().firstAudible.p50).toBe(500)
    })

    it('the two numbers in a sample come from two different measurements', async () => {
        vi.spyOn(console, 'info').mockImplementation(() => {})
        const h = driven()
        await h.client.start()
        h.player.audibleMs = 200
        // Stopping the audio costs 40 ms: the silence mark must see that, and the audible mark
        // must not be the same number.
        const stop = h.player.stop.bind(h.player)
        h.player.stop = () => {
            h.advance(40)
            stop()
        }
        h.speak(200)
        h.stopSpeaking()
        h.advance(300)
        h.client.onFrame(segment())

        const report = liveVoiceReport()
        expect(report.silence.p50, 'measured around stopAudio').toBe(40)
        expect(report.firstAudible.p50, 'measured to the first audible sample').toBe(540)
        expect(report.firstAudible.p50).not.toBe(report.silence.p50)
    })

    it('with the recorder off, a whole turn keeps no sample and logs nothing', async () => {
        const spy = vi.spyOn(console, 'info').mockImplementation(() => {})
        setLiveVoiceMetrics(false)
        const h = driven()
        await h.client.start()
        h.player.audibleMs = 200
        h.speak(200)
        h.stopSpeaking()
        h.advance(300)
        h.client.onFrame(segment())

        expect(liveVoiceReport().n).toBe(0)
        expect(spy).not.toHaveBeenCalled()
    })

    it('a turn cut off by the session ending spends no mark on the next session', async () => {
        vi.spyOn(console, 'info').mockImplementation(() => {})
        const h = driven()
        await h.client.start()
        h.player.audibleMs = 200
        h.speak(200)
        h.stopSpeaking()
        // The row goes before the reply arrives: that turn is never measured.
        h.client.stop('left_conversation')

        await h.client.start()
        h.advance(5000)
        h.client.onFrame(segment(2, 0))
        expect(liveVoiceReport().n, 'the stale mark was dropped with the session').toBe(0)
    })
})

/**
 * C5/A — the session's own cycle: an idle ending, a second visit, and a hidden window.
 *
 * What is *not* here, and cannot be: `document.visibilitychange` and React's effect cleanup. This
 * package runs vitest with `environment: 'node'`, so there is no document to dispatch on and no
 * component to mount. The client is asserted on the facts a hidden window would produce — a device
 * the browser took away, and a session that keeps the microphone open while the window is not in
 * focus — and one policy test reads the sources to prove nothing ends a session on `hidden`.
 */
describe('C5/A — an idle ending, a second visit, a hidden window', () => {
    it('T062: an idle ending keeps the reason readable and stops the clock where it stood', async () => {
        const h = driven()
        await h.client.start()
        h.advance(185_000)
        // A frame makes the client render, which is where the clock catches up: the 100 ms tick is
        // a real timer and these tests do not wait on wall-clock time.
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        h.client.onFrame({ live_voice: { type: 'state', state: 'ended', generation: 1, reason: 'idle' } })

        const view = h.last()
        expect(h.client.currentState).toBe('ended')
        expect(view.notice, 'the reason is a sentence, not a code').toBe(
            'live voice ended after a while with nothing said'
        )
        expect(view.notice).not.toMatch(/idle/)
        expect(view.micOpen, 'an idle ending releases the microphone').toBe(false)
        expect(h.microphones(), 'and does it once').toBe(1)
        expect(h.player.closed, 'and closes the audio context').toBe(1)

        const frozen = view.elapsedMs
        expect(frozen, 'three minutes and change').toBeGreaterThanOrEqual(185_000)
        // The clock is not merely reset: it does not advance while the ended row is on screen.
        h.advance(30_000)
        h.client.onFrame({ live_voice: { type: 'error', kind: 'idle', message: 'noise' } })
        expect(h.last().elapsedMs, 'a frozen clock stays frozen').toBe(frozen)
    })

    it('T063: a second visit remembers nothing from the first', async () => {
        const h = driven()
        await h.client.start()
        h.advance(4000)
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        h.client.onFrame({ live_voice: { type: 'state', state: 'ended', generation: 1, reason: 'idle' } })
        expect(h.last().notice).not.toBeNull()
        expect(h.last().elapsedMs).toBeGreaterThan(0)

        await h.client.start()
        const fresh = h.last()
        expect(fresh.notice, 'the old reason is not news about this session').toBeNull()
        expect(fresh.elapsedMs).toBe(0)
        expect(fresh.micOpen).toBe(true)
        expect(h.client.currentState).toBe('listening')
    })

    it('F4: a new call does not open showing what the last one heard', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({
            live_voice: { type: 'transcript', utterance_id: 'u-1', text: 'what time is standup', submitted: true },
        })
        expect(h.last().transcript).toBe('what time is standup')
        h.client.stop('pressed_again')

        await h.client.start()
        // The comment in `start()` promised this and the code did not do it.
        expect(h.last().transcript).toBe('')
    })

    it('F4: the transcript follows each utterance while the call runs', async () => {
        const h = driven()
        await h.client.start()
        for (const [id, text] of [
            ['u-1', 'first thing I said'],
            ['u-2', 'second thing I said'],
        ] as const) {
            h.client.onFrame({ live_voice: { type: 'transcript', utterance_id: id, text, submitted: true } })
            expect(h.last().transcript).toBe(text)
            expect(h.client.currentState, 'and the call is still live').toBe('listening')
        }
    })

    it('T063: a new session reaches connecting before the device is open', async () => {
        // The permission prompt, held open: the promise resolves only when the test says so.
        const pending: ((value: { stop(): void; open: boolean }) => void)[] = []
        const view: LiveVoiceView[] = []
        const client = new LiveVoiceClient({
            send: () => true,
            player: new SilentPlayer(),
            openCapture: () =>
                new Promise((resolve) => {
                    pending.push(resolve)
                }),
            onView: (next) => view.push(next),
            now: () => 1000,
        })
        void client.start()
        // The permission prompt is still up: the person sees `connecting`, and nothing else.
        expect(client.currentState).toBe('connecting')
        expect(view[view.length - 1].notice).toBeNull()
        pending[0]?.({ stop: () => {}, open: true })
    })

    it('T065: the microphone is taken away mid-session, and the reason is readable', async () => {
        const h = driven()
        await h.client.start()
        h.advance(2000)
        // What a suspended or revoked device produces: `capture.ts` reports it, and the session
        // must not pretend to still be listening.
        h.lost('the microphone was taken away')

        const view = h.last()
        expect(h.client.currentState).toBe('failed')
        expect(view.notice).toBe('the microphone was taken away')
        expect(view.micOpen).toBe(false)
        expect(h.microphones()).toBe(1)
    })

    it('T065: nothing ends a session because the window was hidden', () => {
        // A policy test, and it is the only honest form of this one here: there is no document to
        // dispatch `visibilitychange` on. FR-038 wants the microphone alive while the window is in
        // the background; the one hide event the surface listens for is `pagehide`, which fires on
        // navigation and page close, not on a hidden tab.
        const control = readFileSync('app/screens/chat/LiveVoiceControl.tsx', 'utf8')
        const session = readFileSync('app/lib/liveVoice/session.ts', 'utf8')
        for (const source of [control, session]) {
            expect(source).not.toContain('visibilitychange')
            expect(source).not.toMatch(/document\.hidden/)
        }
        expect(control, 'pagehide is the hide event the surface uses').toContain("'pagehide'")
    })
})

/**
 * C5/B — the failures someone can meet and recover from without reloading the page.
 */
describe('C5/B — after a failure, the next attempt works', () => {
    it('T071: Try again after a lost device opens a new microphone, without a reload', async () => {
        const h = driven()
        await h.client.start()
        expect(h.last().micOpen, 'the first device is open').toBe(true)
        expect(h.microphones(), 'nothing released yet').toBe(0)
        h.lost('the microphone was taken away')
        expect(h.client.currentState).toBe('failed')
        expect(h.last().notice).toBe('the microphone was taken away')
        expect(h.microphones(), 'the failed session released its device').toBe(1)
        expect(h.player.closed, 'the failed session closed its audio context').toBe(1)

        // What the row's `Try again` does: the same `start()` the pill calls.
        await h.client.start()
        expect(h.client.currentState).toBe('listening')
        expect(h.last().micOpen, 'a second device is open').toBe(true)
        // `microphones()` counts releases: the first device was let go exactly once, and the new
        // one is still open — which is what proves a second capture was really requested.
        expect(h.microphones(), 'the first device was released once, not twice').toBe(1)
        expect(h.last().notice, 'the old reason does not follow the new session').toBeNull()
        expect(h.player.closed, 'the closed context was not reused').toBe(1)

        h.client.stop('pressed_again')
        expect(h.player.closed, 'and the second session closes its own context').toBe(2)
    })

    it('T071: a failed session that could not open a device is offered the same way', async () => {
        const views: LiveVoiceView[] = []
        let attempts = 0
        const player = new SilentPlayer()
        const client = new LiveVoiceClient({
            send: () => true,
            player,
            openCapture: async () => {
                attempts += 1
                if (attempts === 1) throw new Error('no microphone was found')
                return { stop: () => {}, open: true }
            },
            onView: (view) => views.push(view),
            now: () => 1000,
        })
        await client.start()
        expect(client.currentState).toBe('failed')
        expect(views[views.length - 1].notice).toBe('no microphone was found')

        await client.start()
        expect(client.currentState).toBe('listening')
        expect(attempts, 'the device is asked for again, not remembered as refused').toBe(2)
    })

    it('T073: an empty transcript and an unintelligible one are two different notices', async () => {
        const h = driven()
        await h.client.start()

        // FR-024: the utterance was too short to be a word. The server says so with a transcript
        // frame that was not submitted, and the client says why.
        h.client.onFrame({
            live_voice: { type: 'transcript', utterance_id: 'u-1', text: '', submitted: false },
        })
        expect(h.last().notice).toBe('that was too short to send')

        // FR-031: the recogniser heard nothing it could use. That arrives as an `error` frame
        // carrying its own kind, so the two cases are already distinguishable by the frame type —
        // no `kind` had to be invented on the transcript frame.
        h.client.onFrame({
            live_voice: {
                type: 'error',
                kind: 'unintelligible',
                message: 'that was not intelligible; say it again',
            },
        })
        expect(h.last().notice).toBe('that was not intelligible; say it again')
        expect(h.last().notice).not.toBe('that was too short to send')
        // Neither is an ending: the session is still listening.
        expect(h.client.currentState).toBe('listening')
    })
})

/**
 * N1 — `connecting` has a door out.
 *
 * `startFrame` leaves before the microphone is awaited, so the server holds a session from that
 * moment. If the device never opened — a permission prompt left on screen, a browser that never
 * answers — the row sat on `Connecting…` for ever and the server kept a session nobody could use.
 */
describe('N1 — a device that never opens does not hang the session', () => {
    /** A client whose device opens only when the test says so. */
    function pendingCapture(deadlineMs: number) {
        const sent: object[] = []
        const views: LiveVoiceView[] = []
        let settle: ((value: { stop(): void; open: boolean }) => void) | null = null
        let stopped = 0
        const client = new LiveVoiceClient({
            send: (frame) => {
                sent.push(frame)
                return true
            },
            player: new SilentPlayer(),
            openCapture: () =>
                new Promise((resolve) => {
                    settle = resolve
                }),
            onView: (view) => views.push(view),
            now: () => 1000,
            connectDeadlineMs: deadlineMs,
        })
        return {
            client,
            sent,
            views,
            last: () => views[views.length - 1],
            stopped: () => stopped,
            open: () => settle?.({ stop: () => (stopped += 1), open: true }),
            keys: () => sent.map((frame) => Object.keys(frame)[0]),
        }
    }

    it('gives up on the device, says why, and tells the server', async () => {
        const h = pendingCapture(20)
        void h.client.start()
        expect(h.client.currentState, 'the prompt is still up').toBe('connecting')

        await new Promise((resolve) => setTimeout(resolve, 60))

        expect(h.client.currentState).toBe('failed')
        expect(h.last().notice, 'a sentence, not a code').toContain('the microphone did not open')
        expect(h.last().micOpen).toBe(false)
        // The server's session started with the frame that left before the device was asked for;
        // a client that never opens its microphone must not leave it standing.
        expect(h.keys(), 'the server is told').toContain('live_voice_stop')
    })

    it('a device that arrives too late is stopped, not kept', async () => {
        const h = pendingCapture(20)
        void h.client.start()
        await new Promise((resolve) => setTimeout(resolve, 60))
        expect(h.client.currentState).toBe('failed')

        // The person finally clicks Allow: the device is real, and nothing is driving it.
        h.open()
        await new Promise((resolve) => setTimeout(resolve, 0))

        expect(h.stopped(), 'the microphone is released at once').toBe(1)
        expect(h.client.currentState, 'and the session stays over').toBe('failed')
        expect(h.last().micOpen).toBe(false)
    })

    it('a device that opens in time is not disturbed by the deadline', async () => {
        const h = pendingCapture(20)
        const starting = h.client.start()
        h.open()
        await starting
        expect(h.client.currentState).toBe('listening')
        expect(h.last().micOpen).toBe(true)

        // Past the deadline: the timer was cleared, so nothing changes and nothing is released.
        await new Promise((resolve) => setTimeout(resolve, 60))
        expect(h.client.currentState).toBe('listening')
        expect(h.stopped()).toBe(0)
        expect(h.keys()).not.toContain('live_voice_stop')
    })
})

/**
 * N1 regression (`08cf067`) — "Listening + mic off", reported from a live session.
 *
 * The server answers a start frame with `listening` as soon as it has accepted it, which can be
 * before the microphone resolves. That echo used to move the session off `connecting`, and the N1
 * guard then treated the device that arrived a moment later as "too late" and closed it: the row
 * said `Listening`, the chip said `mic off`, the clock never started, and no audio was ever sent.
 */
describe('N1 regression — a listing echo that overtakes the device must not kill it', () => {
    /** A client whose device resolves only when the test releases it. */
    function slowDevice() {
        const sent: object[] = []
        const views: LiveVoiceView[] = []
        let settle: ((value: { stop(): void; open: boolean }) => void) | null = null
        let stopped = 0
        let clock = 1000
        const client = new LiveVoiceClient({
            send: (frame) => {
                sent.push(frame)
                return true
            },
            player: new SilentPlayer(),
            openCapture: () =>
                new Promise((resolve) => {
                    settle = resolve
                }),
            onView: (view) => views.push(view),
            now: () => clock,
            connectDeadlineMs: 5_000,
        })
        return {
            client,
            stopped: () => stopped,
            last: () => views[views.length - 1],
            open: () => settle?.({ stop: () => (stopped += 1), open: true }),
            advance: (ms: number) => {
                clock += ms
            },
        }
    }

    it('keeps the device when the server says listening before the microphone opens', async () => {
        const h = slowDevice()
        const starting = h.client.start()
        expect(h.client.currentState, 'the permission prompt is still up').toBe('connecting')

        // What the server really sends: it accepted the start frame, so it is listening already.
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        expect(h.client.currentState, 'the echo is not news, the door is start()').toBe('connecting')

        h.open()
        await starting
        expect(h.stopped(), 'the device is attached, not thrown away').toBe(0)
        expect(h.client.currentState).toBe('listening')
        expect(h.last().micOpen, 'and the chip can say mic on').toBe(true)

        // The clock starts with the device, so a live session is not frozen at 0:00.
        h.advance(1200)
        h.client.onFrame({ live_voice: { type: 'transcript', utterance_id: 'u-1', text: 'halo', submitted: true } })
        expect(h.last().elapsedMs).toBeGreaterThanOrEqual(1200)
    })

    it('a real reconnect still moves the session on', async () => {
        // The echo is only ignored while `connecting` with no device. A dropped socket that comes
        // back is genuine news, and that path must not be caught by the same guard.
        const h = slowDevice()
        const starting = h.client.start()
        h.open()
        await starting
        expect(h.client.currentState).toBe('listening')

        h.client.onSocketState('reconnecting')
        expect(h.client.currentState).toBe('reconnecting')
        h.client.onSocketState('open')
        expect(h.client.currentState, 'back on the same session').toBe('listening')
    })
})

/**
 * D2 — the deadline must survive the server's `listening` echo.
 *
 * The echo is what made the original report ("Listening" with `mic off`) possible, so the deadline
 * is checked with the echo in the picture: the row must still give up, say why, and tell the server
 * exactly once. Gating the deadline on the state word instead of on the missing device is what
 * would let a prompt nobody answered hang for ever.
 */
describe('D2 — a prompt nobody answers still ends, even after the listening echo', () => {
    function pendingDevice(deadlineMs: number) {
        const sent: object[] = []
        const views: LiveVoiceView[] = []
        let settle: ((value: { stop(): void; open: boolean }) => void) | null = null
        const client = new LiveVoiceClient({
            send: (frame) => {
                sent.push(frame)
                return true
            },
            player: new SilentPlayer(),
            openCapture: () =>
                new Promise((resolve) => {
                    settle = resolve
                }),
            onView: (view) => views.push(view),
            now: () => 1000,
            connectDeadlineMs: deadlineMs,
        })
        return {
            client,
            last: () => views[views.length - 1],
            open: () => settle?.({ stop: () => {}, open: true }),
            stops: () => sent.filter((frame) => 'live_voice_stop' in frame).length,
        }
    }

    it('fails with a readable reason and one stop frame', async () => {
        const h = pendingDevice(20)
        void h.client.start()
        // The server accepted the start frame, so it is listening already — the echo that started
        // this whole thread.
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })

        await new Promise((resolve) => setTimeout(resolve, 60))

        expect(h.client.currentState).toBe('failed')
        expect(h.last().notice).toContain('the microphone did not open')
        expect(h.last().micOpen).toBe(false)
        expect(h.stops(), 'exactly one ending, not two').toBe(1)
    })

    it('an ending the person asked for is not repeated by the deadline', async () => {
        // Pressing End while the prompt is up must not turn into a second `live_voice_stop`
        // when the timer fires a moment later.
        const h = pendingDevice(20)
        void h.client.start()
        h.client.stop('pressed_again')
        expect(h.stops()).toBe(1)

        await new Promise((resolve) => setTimeout(resolve, 60))
        expect(h.stops(), 'the deadline stays quiet').toBe(1)
        expect(h.client.currentState).toBe('ended')
    })
})

/**
 * The level behind the meter. It was written as the constant 0, so the bar existed and could never
 * move; these three assertions tie the number to the audio that produced it.
 */
describe('the level meter follows the input', () => {
    it('a loud window lifts the level off zero', async () => {
        const h = ordered()
        await h.client.start()
        expect(h.client.currentView.level, 'nothing heard yet').toBe(0)
        h.voice(20)
        expect(h.client.currentView.level).toBeGreaterThan(0.05)
    })

    it('a silent window afterwards pulls the level back down', async () => {
        const h = ordered()
        await h.client.start()
        h.voice(200)
        const loud = h.client.currentView.level
        expect(loud).toBeGreaterThan(0.05)
        h.silence(200)
        const quiet = h.client.currentView.level
        expect(quiet).toBeLessThan(loud)
        expect(quiet).toBeGreaterThanOrEqual(0)
    })

    it('an ending leaves the meter at zero', async () => {
        // The row must not keep a bar standing at whatever the last window was once the
        // microphone is gone.
        const h = ordered()
        await h.client.start()
        h.voice(200)
        expect(h.client.currentView.level).toBeGreaterThan(0)
        h.client.stop('pressed_again')
        expect(h.client.currentState).toBe('ended')
        expect(h.client.currentView.level).toBe(0)
    })
})

describe('the blip guard — a knock does not cut a reply (C8)', () => {
    // The report this answers: the agent spoke twice and then only text. The utterance end used to
    // interrupt unconditionally, so a 20 ms blip of desk, chair or AGC closed the utterance, stopped
    // the audio locally and cancelled the turn on the server: the rest of the answer was never
    // spoken, though its text was already saved.
    it('a 20 ms blip leaves the reply alone and sends no utterance end', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        h.client.onFrame(audible)

        // A blip: one voiced window, then the hangover. The gate is raised while the reply is
        // audible, so this level clears it — which is exactly the false positive being rejected.
        h.speak(20, 0.3)
        h.stopSpeaking()

        expect(
            h.sent.filter((frame) => 'live_voice_utterance_end' in frame),
            'a blip is not the end of a sentence'
        ).toHaveLength(0)
        expect(h.last().state, 'the call is still speaking, because nobody interrupted it').toBe(
            'speaking'
        )
    })

    it('a word of 400 ms still ends the utterance, as it did before', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({ live_voice: { type: 'state', state: 'speaking', generation: 1 } })
        h.client.onFrame(audible)

        h.speak(400, 0.3)
        h.stopSpeaking()

        expect(
            h.sent.filter((frame) => 'live_voice_utterance_end' in frame),
            'a person speaking over the reply is how they interrupt it (FR-019)'
        ).toHaveLength(1)
    })

    it('the abandoned utterance is replaced, not closed, by the next speech', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame(audible)
        h.speak(20, 0.3)
        h.stopSpeaking()
        const opened = h.sent.filter((frame) => 'live_voice_speech_start' in frame).length
        h.speak(60, 0.3)
        expect(
            h.sent.filter((frame) => 'live_voice_speech_start' in frame).length,
            'a second blip opens a second utterance rather than reusing a closed one'
        ).toBe(opened + 1)
        expect(h.sent.filter((frame) => 'live_voice_utterance_end' in frame)).toHaveLength(0)
    })
})

describe('V9 — the meter returns between turns', () => {
    it('clearing the transcript gives the middle slot back, and only once it is handed over', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({
            live_voice: {
                type: 'transcript',
                utterance_id: 'u-1',
                text: 'what time is standup',
                submitted: true,
            },
        })
        expect(h.last().transcript).toBe('what time is standup')

        // Nothing has been asked for yet: the clear is the control's call, after adoption, so a
        // render here must leave the words where they are.
        h.client.onFrame({ live_voice: { type: 'state', state: 'listening', generation: 1 } })
        expect(h.last().transcript, 'the words are still the only place they exist').toBe(
            'what time is standup'
        )

        // The control hands them to the thread, then asks for the slot back.
        h.client.transcriptAdopted()
        expect(h.last().transcript).toBe('')
    })

    it('a second turn shows its own words, not the first turn\u2019s', async () => {
        const h = driven()
        await h.client.start()
        h.client.onFrame({
            live_voice: { type: 'transcript', utterance_id: 'u-1', text: 'first', submitted: true },
        })
        h.client.onFrame({
            live_voice: { type: 'transcript', utterance_id: 'u-2', text: 'second', submitted: true },
        })
        expect(h.last().transcript).toBe('second')
    })
})
