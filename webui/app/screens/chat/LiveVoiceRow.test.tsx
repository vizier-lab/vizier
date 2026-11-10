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
    elapsedMs: 0,
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

describe('the agent the call is with (FR-044)', () => {
    it('names the agent in the row, not only in the conversation header', () => {
        const html = render({ state: 'listening', micOpen: true }, { agentName: 'Bro PM' })
        // The requirement is that the row shows it (`spec.md:565`); `spec.md:572` already refused
        // the header as a substitute, by the same reading FR-038 applies to the microphone.
        expect(html).toContain('data-agent')
        expect(html).toContain('Bro PM')
        expect(html).toContain('title="Bro PM"')
        // The whole name travels in the title, because the row's own copy is the truncated one.
        expect(html).toContain('text-ellipsis')
        expect(html).toContain('max-width:96px')
    })

    it('draws no slot at all when there is no name', () => {
        // An empty element that says "the agent" would be worse than no element: it takes width
        // from the transcript and carries nothing.
        expect(render({ state: 'listening' })).not.toContain('data-agent')
        expect(render({ state: 'listening' }, { agentName: '   ' })).not.toContain('data-agent')
    })

    it('is absent from the idle pill, which has one word and one button', () => {
        const html = render({ state: 'off' }, { agentName: 'Bro PM' })
        expect(html).not.toContain('data-agent')
        expect(html).toContain('Start live voice')
    })
})

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

        // The word carries the text shade; `state/live` is for marks and fails 4.5:1 as type.
        expect(rowTone(view({ state: 'waiting' }))).toBe('text-live-text')
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

    it('the state word never uses the mark token, which fails as small type', () => {
        // `state/live` is 3.77:1 on white and 3.43:1 on bg-surface, under the 4.5:1 a 10.5 px word
        // needs; `state/live-text` is 6.44:1 and 5.86:1. Both resolve to the same emerald in the
        // dark theme, so the change costs nothing there.
        for (const state of ['connecting', 'listening', 'waiting', 'speaking', 'interrupted'] as const) {
            expect(rowTone(view({ state })), state).toBe('text-live-text')
        }
        // The microphone glyph is a graphic, not type: the 3:1 mark threshold applies, so it keeps
        // the mark token it was drawn for.
        expect(render({ state: 'speaking', micOpen: true })).toContain('text-live')
    })
})

/**
 * F4 — the transcript on screen while the call runs.
 *
 * It used to be rendered only in `LiveVoiceBar`, which stopped being drawn when the row replaced
 * it: the words were in state, never on screen, and a person saw them only after a refresh as the
 * stored message. These assert the row itself, and that it costs no height.
 */
describe('F4 — the words that were heard', () => {
    it('shows the transcript while the call is live', () => {
        const html = render({ state: 'listening', micOpen: true, transcript: 'what time is the standup' })
        expect(html).toContain('data-transcript')
        expect(html).toContain('what time is the standup')
        // The waveform gives way to it — one flexible slot, not two, so the row keeps its height.
        expect(html).not.toContain('data-bars')
    })

    it('keeps the waveform when nothing has been heard yet', () => {
        const html = render({ state: 'listening', micOpen: true, transcript: '' })
        expect(html).not.toContain('data-transcript')
        expect(html).toContain('data-bars')
    })

    it('an ending still shows its reason rather than the last words', () => {
        const html = render({ state: 'ended', transcript: 'what time is the standup', notice: 'live voice is off' })
        expect(html).toContain('live voice is off')
        expect(html).not.toContain('data-transcript')
    })

    it('never wraps, so the row cannot grow past its 24 px', () => {
        const html = render({ state: 'listening', micOpen: true, transcript: 'a very long sentence '.repeat(20) })
        const slot = html.slice(html.indexOf('data-transcript'))
        const tag = slot.slice(0, slot.indexOf('>'))
        expect(tag).toContain('truncate')
        expect(tag).toContain('whitespace-nowrap')
        expect(tag).toContain('flex-1')
    })
})

describe('T112 — a long name costs width, never height', () => {
    // The DoD in T112: one test with a 40-character name proving the 24 px row still holds and the
    // elements that must never shrink do not move. Geometry is asserted from the markup the row
    // writes: `ROW.height` inline, and `shrink-0` on the four fixed slots.
    const LONG = 'Ketua Tim Riset Produk dan Inovasi Digital'

    it('keeps the row 24 px and the four fixed slots unshrunk', () => {
        const html = render({ state: 'listening', micOpen: true }, { agentName: LONG })
        expect(LONG.length, 'the fixture is the length the task names').toBeGreaterThanOrEqual(40)
        expect(html).toContain('height:24px')
        expect(html).toContain('data-agent')
        expect(html).toContain(LONG)
        // The name is the only slot that gives width back: no wrap, an ellipsis, and a ceiling.
        expect(html).toContain('text-ellipsis')
        expect(html).toContain('whitespace-nowrap')
        expect(html).toContain('max-width:96px')
        // The four that must not shrink still say so.
        expect(html.match(/shrink-0/g)?.length ?? 0).toBeGreaterThanOrEqual(4)
    })

    it('the transcript keeps the space the name does not use', () => {
        const withName = render(
            { state: 'listening', micOpen: true, transcript: 'what time is standup' },
            { agentName: LONG }
        )
        // The middle slot is `flex-1`: it takes what is left rather than a fixed width, which is
        // what lets a long name cost the transcript width but never the row its height.
        expect(withName).toContain('data-transcript')
        expect(withName).toContain('flex-1')
    })
})
