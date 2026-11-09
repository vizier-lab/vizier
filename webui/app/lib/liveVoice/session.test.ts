import { describe, expect, it, vi } from 'vitest'
import { LiveVoiceClient, humanReason, type LiveVoiceDeps, type LiveVoiceView } from './session'
import { SilentPlayer } from './player'
import { stateWord } from '../../screens/chat/LiveVoiceBar'
import type { SessionState } from './frames'

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

    it('a typed message interrupts with its own reason and is not spoken (FR-018 withdrawn)', async () => {
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
        ] as const
        for (const reason of reasons) {
            const wording = humanReason(reason)
            expect(wording).not.toBe(reason)
            expect(wording.length).toBeGreaterThan(8)
        }
        // The one pair that must not share a wording.
        expect(humanReason('idle')).not.toBe(humanReason('connection_lost'))
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
    const client = new LiveVoiceClient({
        send: (frame) => {
            sent.push(frame)
            return options.sendReturns ? options.sendReturns(frame) : true
        },
        player,
        openCapture: async (callbacks) => {
            capture = callbacks
            return { stop: () => {}, open: true }
        },
        onView: (view) => views.push(view),
        now: () => clock,
    })
    return {
        client,
        sent,
        player,
        views,
        last: () => views[views.length - 1],
        advance: (ms: number) => {
            clock += ms
        },
        /** One 20 ms window of voice, which is what the gate decides on. */
        speak: (ms = 200) => {
            const window = new Float32Array(320)
            for (let i = 0; i < window.length; i++) window[i] = i % 2 === 0 ? 0.2 : -0.2
            for (let elapsed = 0; elapsed < ms; elapsed += 20) capture?.onWindow(window)
        },
        /** Silence for the hangover, which ends the utterance. */
        stopSpeaking: () => {
            const silent = new Float32Array(320)
            for (let elapsed = 0; elapsed < 900; elapsed += 20) capture?.onWindow(silent)
        },
    }
}

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

    it('a burst too short to be a word still ends the utterance without cancelling nothing', async () => {
        const h = ordered()
        await h.client.start()
        h.client.onFrame(audible)
        // 100 ms of voice, then silence: the server decides too-short (FR-024); the client's job
        // is only to close the utterance and stop the reply it was talking over.
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
