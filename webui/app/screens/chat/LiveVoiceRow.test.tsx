/**
 * The live voice row, rendered on the server (`specs/055-live-voice`, surface B1-1).
 *
 * A server render is enough for everything this component decides: which shape it takes, which
 * word it says, which colour that word carries, and what is inside the markup. Nothing here needs
 * a layout, which is why the waveform's bar count has a default and a `data-bars` hook for the
 * real measurement.
 */

import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { LiveVoiceRow, rowTone, rowWord } from './LiveVoiceRow'
import type { LiveVoiceView } from '../../lib/liveVoice/session'

const view = (patch: Partial<LiveVoiceView> = {}): LiveVoiceView => ({
    state: 'off',
    thinking: false,
    thinkingMs: 0,
    level: 0,
    transcript: '',
    notice: null,
    micOpen: false,
    ...patch,
})

const render = (patch: Partial<LiveVoiceView> = {}, extra: Record<string, unknown> = {}) =>
    renderToString(
        <LiveVoiceRow
            view={view(patch)}
            capability
            onStart={() => {}}
            onEnd={() => {}}
            onRestart={() => {}}
            {...extra}
        />
    )

describe('the idle row', () => {
    it('is one pill and nothing else — live voice never appears by itself', () => {
        const html = render()
        expect(html).toContain('Live voice')
        expect(html).toContain('Start live voice')
        // No session furniture before the session exists.
        expect(html).not.toContain('mic on')
        expect(html).not.toContain('End')
        expect(html).not.toContain('data-bars')
        expect(html).not.toMatch(/data-state=/)
    })

    it('offers a disabled pill with the reason when the agent cannot hold a session', () => {
        const html = render({}, { capability: false, missing: 'voice' })
        expect(html).toContain('a voice is not configured for this agent')
        expect(html).not.toContain('Start live voice')
    })
})

describe('the live row', () => {
    it('says one word per state', () => {
        const words: [LiveVoiceView['state'], string][] = [
            ['connecting', 'Connecting…'],
            ['waiting', 'Thinking'],
            ['listening', 'Listening'],
            ['speaking', 'Speaking'],
            ['interrupted', 'Interrupted'],
            ['reconnecting', 'Reconnecting'],
            ['ended', 'Ended'],
            ['failed', 'Failed'],
        ]
        for (const [state, word] of words) {
            expect(rowWord(view({ state })), state).toBe(word)
            expect(render({ state })).toContain(word)
        }
    })

    it('tells thinking from reconnecting by colour and marker, not by wording alone', () => {
        const thinking = render({ state: 'waiting', thinking: true, micOpen: true })
        const reconnecting = render({ state: 'reconnecting', micOpen: true })

        expect(rowTone(view({ state: 'waiting' }))).toBe('text-live')
        expect(rowTone(view({ state: 'reconnecting' }))).toBe('text-retrying-text')
        expect(rowTone(view({ state: 'waiting' }))).not.toBe(rowTone(view({ state: 'reconnecting' })))

        // The marker survives reduced motion: three dots versus one.
        expect(thinking).toContain('•••')
        expect(reconnecting).not.toContain('•••')
        // And the words are never the same string.
        expect(rowWord(view({ state: 'waiting' }))).not.toBe(rowWord(view({ state: 'reconnecting' })))
    })

    it('always says whether the microphone is on, at every state', () => {
        for (const state of ['connecting', 'listening', 'waiting', 'speaking', 'reconnecting', 'ended', 'failed'] as const) {
            expect(render({ state, micOpen: true }), state).toContain('mic on')
        }
        expect(render({ state: 'listening', micOpen: false })).toContain('mic off')
        // An ending closes the microphone, and the row says so rather than leaving a blank.
        expect(render({ state: 'ended', micOpen: false, notice: 'idle for 3 minutes' })).toContain('mic off')
    })

    it('keeps the reason with an ending, and offers the way back', () => {
        const ended = render({ state: 'ended', notice: 'idle for 3 minutes' })
        expect(ended).toContain('idle for 3 minutes')
        expect(ended).toContain('Start again')
        expect(ended).not.toContain('>End<')

        const failed = render({ state: 'failed', notice: 'no voice configured' })
        expect(failed).toContain('no voice configured')
        expect(failed).toContain('Try again')
        expect(failed).not.toContain('Start again')
    })

    it('shows the clock as m:ss and a waveform with a measurable bar count', () => {
        const html = render({ state: 'speaking', micOpen: true }, { elapsedSeconds: 84 })
        expect(html).toContain('1:24')
        const bars = /data-bars="(\d+)"/.exec(html)
        expect(bars, 'the waveform exposes its bar count').not.toBeNull()
        const count = Number(bars![1])
        expect(count).toBeGreaterThanOrEqual(12)
        expect(count).toBeLessThanOrEqual(39)
    })

    it('carries no interrupt control: the only way to stop it is to speak', () => {
        for (const state of ['off', 'connecting', 'listening', 'waiting', 'speaking', 'reconnecting', 'ended', 'failed'] as const) {
            const html = render({ state, micOpen: true, notice: 'idle for 3 minutes' })
            expect(html.toLowerCase(), state).not.toContain('interrupt')
        }
    })

    it('never wraps: every item that must not shrink is nowrap', () => {
        const html = render({ state: 'speaking', micOpen: true })
        // State, mic, timer and the End control each carry the rule.
        expect(html.match(/whitespace-nowrap/g)?.length ?? 0).toBeGreaterThanOrEqual(4)
        expect(html).not.toContain('text-ellipsis')
    })
})
