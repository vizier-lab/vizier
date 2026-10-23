import { describe, expect, it } from 'vitest'
import type { ConnectedProvider } from '../api/endpoints/providers'
import { credentialText, usageText } from './providers'

const base: ConnectedProvider = {
    id: 'anthropic', name: 'Anthropic', origin: 'catalogue', kind: 'cloud', model_count: 41, fields: ['api_key'],
    has_api_key: true, has_access_token: false, used_by: { chat: 3, stt: 0, tts: 0, image_gen: 0, embedding: 0 },
}

describe('usageText', () => {
    it('counts agents and models', () => {
        expect(usageText(base.used_by, 41)).toBe('3 agents · 41 models')
        expect(usageText({ chat: 1, stt: 0, tts: 0, image_gen: 0, embedding: 0 }, 1)).toBe('1 agent · 1 model')
    })
    it('says so when nothing uses it', () => {
        expect(usageText({ chat: 0, stt: 0, tts: 0, image_gen: 0, embedding: 0 }, 0)).toBe('not used yet')
    })
})

describe('credentialText', () => {
    it('tells a saved key from one in the environment', () => {
        expect(credentialText({ ...base, source: 'saved' })).toBe('Saved key')
        expect(credentialText({ ...base, source: 'env:ANTHROPIC_API_KEY' })).toBe('Key from ANTHROPIC_API_KEY')
    })
    it('shows a local address and no key', () => {
        expect(credentialText({ ...base, has_api_key: false, fields: ['endpoint'], endpoint: 'http://localhost:11434' })).toBe('http://localhost:11434')
    })
    it('shows a sign-in', () => {
        expect(credentialText({ ...base, has_api_key: false, has_access_token: true, account_id: 'ana@x.io' })).toBe('Signed in as ana@x.io')
    })
})
