import { afterEach, describe, expect, it } from 'vitest'
import { LiveVoiceClient, type LiveVoiceView } from './session'
import { SilentPlayer } from './player'
import {
    bindLiveVoice,
    clearLiveVoice,
    endLiveVoice,
    forwardLiveVoiceFrame,
    forwardLiveVoiceState,
} from './control'

/**
 * T090 — the composer's surface is hidden in two ways by React (`pagehide`, then the unmount
 * cleanup), and both go through `endLiveVoice`. These are the legs that can be run here: the
 * binding and the client it hands frames to. What cannot be run without a DOM is React invoking the
 * effect cleanup itself; see the note at the end of this file.
 */

const AGENT = 'oz'
const TOPIC = 't1'

/** A real client with a fake socket, a fake microphone and a silent player. */
function client() {
    const sent: object[] = []
    const views: LiveVoiceView[] = []
    const player = new SilentPlayer()
    let microphones = 0
    const live = new LiveVoiceClient({
        send: (frame) => {
            sent.push(frame)
            return true
        },
        player,
        openCapture: async () => ({
            stop: () => {
                microphones += 1
            },
            open: true,
        }),
        onView: (view) => views.push(view),
        now: () => 1000,
    })
    return {
        live,
        sent,
        player,
        views,
        microphones: () => microphones,
        last: () => views[views.length - 1],
        stopFrames: () => sent.filter((frame) => 'live_voice_stop' in frame).length,
    }
}

const frame = (state: string) => ({ live_voice: { type: 'state', state, generation: 1 } })

afterEach(() => clearLiveVoice())

describe('T090 — the row leaving through the binding', () => {
    it('unmounting ends the session and unregisters it, so a late frame has nowhere to land', async () => {
        const h = client()
        bindLiveVoice(AGENT, TOPIC, h.live, () => {})
        await h.live.start()
        forwardLiveVoiceFrame(AGENT, TOPIC, frame('listening'))
        expect(h.live.currentState).toBe('listening')

        // What the unmount cleanup does.
        endLiveVoice(AGENT, TOPIC, h.live, 'left_conversation')
        expect(h.live.currentState).toBe('ended')
        expect(h.microphones(), 'the tracks are stopped once').toBe(1)
        expect(h.player.closed, 'the audio context is closed').toBe(1)
        expect(h.last().micOpen).toBe(false)

        // The entry is gone: the next frame reaches nothing at all.
        const seen = h.views.length
        expect(forwardLiveVoiceFrame(AGENT, TOPIC, frame('speaking'))).toBe(true)
        expect(h.views.length, 'a frame after the unmount changes no view').toBe(seen)
        expect(h.live.currentState).toBe('ended')
    })

    it('a page hide followed by an unmount releases once, and sends one ending', async () => {
        const h = client()
        bindLiveVoice(AGENT, TOPIC, h.live, () => {})
        await h.live.start()

        // `pagehide` fires, then React unmounts the composer: two calls, one ending.
        endLiveVoice(AGENT, TOPIC, h.live, 'left_conversation')
        endLiveVoice(AGENT, TOPIC, h.live, 'left_conversation')

        expect(h.stopFrames()).toBe(1)
        expect(h.microphones()).toBe(1)
        expect(h.player.closed).toBe(1)
    })

    it('a newer client on the same conversation survives the old surface leaving', async () => {
        const first = client()
        const second = client()
        bindLiveVoice(AGENT, TOPIC, first.live, () => {})
        await first.live.start()
        bindLiveVoice(AGENT, TOPIC, second.live, () => {})
        await second.live.start()

        // The old composer unmounts after the new one mounted: it must not take the new entry.
        endLiveVoice(AGENT, TOPIC, first.live, 'left_conversation')
        forwardLiveVoiceFrame(AGENT, TOPIC, frame('listening'))
        expect(second.live.currentState).toBe('listening')
        expect(first.live.currentState).toBe('ended')
    })

    it('a socket event after the row is gone is a no-op, not a throw', async () => {
        const h = client()
        bindLiveVoice(AGENT, TOPIC, h.live, () => {})
        await h.live.start()
        endLiveVoice(AGENT, TOPIC, h.live, 'left_conversation')
        expect(() => forwardLiveVoiceState(AGENT, TOPIC, 'reconnecting')).not.toThrow()
        expect(h.live.currentState).toBe('ended')
    })

    it('ending with no client at all is safe', () => {
        // The `pagehide` handler reads `client.current`, which is null until the row is used.
        expect(() => endLiveVoice(AGENT, TOPIC, null)).not.toThrow()
    })
})

/**
 * NOT RUN here: React's own effect cleanup, i.e. rendering `LiveVoiceControl` and unmounting it, or
 * dispatching `pagehide`. This package runs vitest with `environment: 'node'` and has no DOM
 * library (no jsdom, no happy-dom, no @testing-library/react), so a component cannot be mounted.
 * Adding one is a dependency decision for the Lead, not a change to make inside a test ticket. The
 * seam both handlers call is `endLiveVoice`, and it is covered above with a real client.
 */
