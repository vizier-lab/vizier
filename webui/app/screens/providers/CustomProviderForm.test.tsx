import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { CustomProvider } from '../../api/endpoints/providers'
import { CustomProviderForm, endpointFor, withoutEndpoint } from './CustomProviderForm'

function custom(protocol: CustomProvider['protocol'], base_url: string): CustomProvider {
    return {
        id: 'home',
        name: 'Home gateway',
        protocol,
        base_url,
        has_api_key: true,
        headers: [],
        models: [],
        used_by: { chat: 0, stt: 0, tts: 0, image_gen: 0, embedding: 0 },
    }
}

function draw(provider?: CustomProvider) {
    return renderToString(<CustomProviderForm provider={provider} onDone={() => {}} onCancel={() => {}} />)
}

describe('endpointFor', () => {
    it('names the path the transport appends, per protocol', () => {
        expect(endpointFor('openai_compatible', 'https://oni.lio.id/v1')).toBe('/chat/completions')
        expect(endpointFor('openai_compatible', '')).toBe('/chat/completions')
        expect(endpointFor('anthropic', 'https://api.anthropic.com/v1')).toBe('/messages')
        expect(endpointFor('anthropic', 'https://api.anthropic.com/v1/')).toBe('/messages')
    })

    it('adds /v1 for an Anthropic base that carries no version prefix', () => {
        expect(endpointFor('anthropic', 'https://proxy.test')).toBe('/v1/messages')
        expect(endpointFor('anthropic', 'https://proxy.test/anthropic')).toBe('/v1/messages')
    })
})

describe('withoutEndpoint', () => {
    it('keeps a base URL and drops its trailing slash', () => {
        expect(withoutEndpoint('https://oni.lio.id/v1')).toBe('https://oni.lio.id/v1')
        expect(withoutEndpoint('https://oni.lio.id/v1/')).toBe('https://oni.lio.id/v1')
        expect(withoutEndpoint('  http://10.0.0.8:8000/v1  ')).toBe('http://10.0.0.8:8000/v1')
    })

    it('strips an endpoint a pasted curl line carried', () => {
        expect(withoutEndpoint('https://oni.lio.id/v1/chat/completions')).toBe('https://oni.lio.id/v1')
        expect(withoutEndpoint('https://api.anthropic.com/v1/messages')).toBe('https://api.anthropic.com/v1')
        expect(withoutEndpoint('https://gateway.test/openai/v1/responses')).toBe('https://gateway.test/openai/v1')
    })

    it('leaves a path that only looks like one alone', () => {
        expect(withoutEndpoint('https://gateway.test/openai/v2')).toBe('https://gateway.test/openai/v2')
        expect(withoutEndpoint('')).toBe('')
    })
})

describe('the Base URL field', () => {
    it('shows the tail Vizier appends as a panel, not as part of the value', () => {
        const html = draw(custom('openai_compatible', 'https://oni.lio.id/v1'))
        expect(html).toContain('value="https://oni.lio.id/v1"')
        expect(html).toContain('>/chat/completions</span>')
        expect(html).not.toContain('https://oni.lio.id/v1/chat/completions')
        expect(html).toContain('Vizier adds /chat/completions itself')
    })

    it('changes the panel with the protocol', () => {
        expect(draw(custom('anthropic', 'https://proxy.test/v1'))).toContain('>/messages</span>')
        expect(draw(custom('anthropic', 'https://proxy.test'))).toContain('>/v1/messages</span>')
    })

    it('repairs a saved value that already carried the endpoint', () => {
        const html = draw(custom('openai_compatible', 'https://oni.lio.id/v1/chat/completions'))
        expect(html).toContain('value="https://oni.lio.id/v1"')
        expect(html).not.toContain('https://oni.lio.id/v1/chat/completions')
    })
})
