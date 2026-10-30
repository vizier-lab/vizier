import { describe, expect, it } from 'vitest'
import type { ConnectedProvider } from '../api/endpoints/providers'
import { credentialText } from './providers'

const base: ConnectedProvider = {
    id: 'amazon-bedrock',
    name: 'Amazon Bedrock',
    origin: 'catalogue',
    kind: 'cloud',
    model_count: 200,
    fields: ['access_key_id', 'secret_access_key', 'session_token', 'region'],
    source: 'saved',
    has_api_key: false,
    has_access_token: false,
    used_by: { chat: 0, stt: 0, tts: 0, image_gen: 0, embedding: 0 },
}

describe('the Bedrock connection summary', () => {
    it('shows the saved key pair and the region, never a secret', () => {
        const text = credentialText({
            ...base,
            access_key_id: 'AKIAEXAMPLE',
            has_secret_access_key: true,
            has_session_token: false,
            region: 'us-east-1',
        })
        expect(text).toBe('Saved keys · us-east-1')
        expect(text).not.toContain('AKIAEXAMPLE')
    })

    it('names the environment variable that supplied the keys', () => {
        expect(
            credentialText({
                ...base,
                source: 'env:AWS_ACCESS_KEY_ID',
                access_key_id: 'AKIAENV',
                has_secret_access_key: true,
                region: 'eu-west-1',
            })
        ).toBe('Keys from AWS_ACCESS_KEY_ID · eu-west-1')
    })

    it('says a half-configured connection has no secret key', () => {
        expect(
            credentialText({ ...base, source: null, access_key_id: 'AKIAHALF', has_secret_access_key: false, region: 'us-east-1' })
        ).toBe('no secret key · us-east-1')
    })
})
