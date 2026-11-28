import { describe, expect, it } from 'vitest'
import type { ProviderSummary } from '../api/endpoints/catalogue'
import { mediaProviderRows, mediaProviderSections, needsConnecting } from './mediaProviders'

const entry = (id: string, status: 'connected' | 'missing', source: string | null = null): ProviderSummary =>
    ({ id, name: id.toUpperCase(), origin: 'catalogue', kind: 'media', connection: { status, source } }) as ProviderSummary

describe('mediaProviderRows', () => {
    it('matches setting values to their catalogue entries by credential id', () => {
        const rows = mediaProviderRows(['gemini', 'bedrock'], [entry('google', 'connected', 'saved'), entry('amazon-bedrock', 'missing')])
        expect(rows.map((r) => [r.value, r.entry?.id, r.connected])).toEqual([
            ['gemini', 'google', true],
            ['bedrock', 'amazon-bedrock', false],
        ])
    })

    it('treats the providers that run on this machine as ready', () => {
        const [row] = mediaProviderRows(['kokoro'], [])
        expect(row).toMatchObject({ value: 'kokoro', name: 'Kokoro', local: true, connected: true })
        expect(needsConnecting(row)).toBeUndefined()
    })
})

describe('mediaProviderSections', () => {
    it('lists local, then connected, then the rest, and filters by name', () => {
        const rows = mediaProviderRows(['openai', 'whisper', 'groq'], [entry('openai', 'missing'), entry('groq', 'connected', 'env:GROQ_API_KEY')])
        expect(mediaProviderSections(rows, '').map((s) => s.items.map((r) => r.value))).toEqual([['whisper'], ['groq'], ['openai']])
        expect(mediaProviderSections(rows, 'open').flatMap((s) => s.items.map((r) => r.value))).toEqual(['openai'])
    })
})

describe('needsConnecting', () => {
    it('asks for credentials only for a missing catalogue provider', () => {
        const [missing, done] = mediaProviderRows(['openai', 'groq'], [entry('openai', 'missing'), entry('groq', 'connected', 'saved')])
        expect(needsConnecting(missing)?.id).toBe('openai')
        expect(needsConnecting(done)).toBeUndefined()
    })
})
