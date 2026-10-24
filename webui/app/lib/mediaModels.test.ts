import { describe, expect, it } from 'vitest'
import type { MediaModels, MediaModelSummary } from '../api/endpoints/catalogue'
import {
    ageOf,
    defaultModelOf,
    defaultVoiceOf,
    filterModels,
    modelSections,
    sourceLine,
    triggerLabel,
    typedModel,
    typedVoice,
    voiceSections,
    voicesOf,
} from './mediaModels'

const model = (patch: Partial<MediaModelSummary> & { id: string }): MediaModelSummary => ({
    name: patch.id,
    default: false,
    listed: true,
    voices: [],
    aspect_ratios: [],
    resolutions: [],
    ...patch,
})

const speech: MediaModelSummary[] = [
    model({ id: 'ready/kokoro', name: 'Kokoro 82M', default: true, voices: ['af_alloy', 'am_echo'] }),
    model({ id: 'other/voice', name: 'Other Voice', voices: ['v1'] }),
    model({ id: 'third/voice', name: 'Third' }),
]

const listing = (patch: Partial<MediaModels> = {}): MediaModels => ({
    capability: 'speech',
    source: 'bundled',
    fetched_at: '2026-10-24T00:00:00Z',
    checked_at: null,
    default_voice: 'af_alloy',
    refresh: { enabled: true, ttl: '86400s', in_progress: false, last_error: null, failed_at: null },
    models: speech,
    ...patch,
})

describe('model picker sections', () => {
    it('puts the default in its own section first and the rest after', () => {
        const sections = modelSections(speech, '')
        expect(sections.map((s) => s.label)).toEqual(['Typed id', 'Default', 'Models'])
        expect(sections[1].items.map((m) => m.id)).toEqual(['ready/kokoro'])
        expect(sections[2].items.map((m) => m.id)).toEqual(['other/voice', 'third/voice'])
        expect(sections[0].items).toEqual([])
    })

    it('filters by name and by id', () => {
        expect(filterModels(speech, 'kokoro').map((m) => m.id)).toEqual(['ready/kokoro'])
        expect(filterModels(speech, 'OTHER/VOICE').map((m) => m.id)).toEqual(['other/voice'])
        expect(filterModels(speech, '  ')).toHaveLength(3)
        expect(filterModels(speech, 'nothing')).toEqual([])
    })

    it('offers a typed id only when it is not already listed', () => {
        expect(typedModel('', speech)).toBeUndefined()
        expect(typedModel('ready/kokoro', speech)).toBeUndefined()
        const typed = typedModel('someone/not-listed', speech)
        expect(typed?.id).toBe('someone/not-listed')
        expect(typed?.name).toBe('Use “someone/not-listed”')
        expect(typed?.typed).toBe(true)

        const sections = modelSections(speech, 'someone/not-listed')
        expect(sections[0].items[0].id).toBe('someone/not-listed')
        expect(sections[1].items).toEqual([])
    })

    it('works before the list has loaded and keeps the documented default visible', () => {
        expect(modelSections(undefined, '').every((s) => s.items.length === 0)).toBe(true)
        expect(defaultModelOf(undefined, 'ready/kokoro')?.id).toBe('ready/kokoro')
        expect(defaultModelOf(speech, 'ignored')?.id).toBe('ready/kokoro')
        const items = modelSections(speech, '').flatMap((s) => s.items)
        expect(items.some((m) => m.id === 'ready/kokoro')).toBe(true)
    })
})

describe('the trigger label', () => {
    it('shows Name · id for a chosen model and Default · name for an empty field', () => {
        expect(triggerLabel(speech, 'other/voice', 'ready/kokoro')).toEqual({ name: 'Other Voice', id: 'other/voice', isDefault: false })
        expect(triggerLabel(speech, '', 'ready/kokoro')).toEqual({ name: 'Kokoro 82M', id: 'ready/kokoro', isDefault: true })
        // Before the list loads, the local defaults map stands in.
        expect(triggerLabel(undefined, '', 'ready/kokoro')).toEqual({ name: 'ready/kokoro', id: undefined, isDefault: true })
        // A typed id that is not listed is shown as typed.
        expect(triggerLabel(speech, 'someone/not-listed', 'ready/kokoro')).toEqual({ name: 'someone/not-listed', id: undefined, isDefault: false })
    })
})

describe('voice picker', () => {
    it('offers the selected model voices, and the default model voices when none is chosen', () => {
        expect(voicesOf(speech, 'other/voice')).toEqual(['v1'])
        expect(voicesOf(speech, '')).toEqual(['af_alloy', 'am_echo'])
        expect(voicesOf(speech, 'someone/not-listed')).toEqual([])
        expect(voicesOf(undefined, 'ready/kokoro')).toEqual([])
    })

    it('marks the default voice and keeps a typed voice', () => {
        const sections = voiceSections(['af_alloy', 'am_echo'], '', 'af_alloy')
        expect(sections.map((s) => s.label)).toEqual(['Typed voice', 'Default', 'Voices'])
        expect(sections[1].items.map((v) => v.id)).toEqual(['af_alloy'])
        expect(sections[2].items.map((v) => v.id)).toEqual(['am_echo'])

        const typed = voiceSections(['af_alloy'], 'my-voice', 'af_alloy')
        expect(typed[0].items[0]).toEqual({ id: 'my-voice', typed: true })
        expect(typedVoice('af_alloy', ['af_alloy'])).toBeUndefined()
    })

    it('falls back to the local voice until the server list loads', () => {
        expect(defaultVoiceOf(undefined, 'af_alloy')).toBe('af_alloy')
        expect(defaultVoiceOf(listing(), 'af_alloy')).toBe('af_alloy')
        expect(defaultVoiceOf(listing({ default_voice: 'am_echo' }), 'af_alloy')).toBe('am_echo')
        expect(defaultVoiceOf(listing({ default_voice: null }), 'af_alloy')).toBe('af_alloy')
    })
})

describe('the source line', () => {
    it('names the source and how old the list is', () => {
        const now = Date.parse('2026-10-26T00:00:00Z')
        expect(sourceLine(listing({ fetched_at: '2026-10-25T00:00:00Z' }), now)).toBe('Bundled · 1 d old')
        expect(sourceLine(listing({ source: 'cached', fetched_at: '2026-10-25T23:59:30Z' }), now)).toBe('Cached · just now')
        expect(sourceLine(undefined, now)).toBeUndefined()
        expect(ageOf('not a date', now)).toBeUndefined()
        expect(ageOf('2026-10-25T23:00:00Z', now)).toBe('1 h old')
    })
})
