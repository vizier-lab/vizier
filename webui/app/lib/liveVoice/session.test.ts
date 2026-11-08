import { describe, expect, it, vi } from 'vitest'
import { LiveVoiceClient, humanReason, type LiveVoiceDeps, type LiveVoiceView } from './session'
import { SilentPlayer } from './player'
import type { SessionState } from './frames'

/** A client wired to a fake socket and a silent player, so every transition is observable. */
function harness(options: { captureFails?: string } = {}) {
    const sent: Record<string, unknown>[] = []
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

