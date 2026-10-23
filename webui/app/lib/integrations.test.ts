import { describe, expect, it } from 'vitest'
import type { IntegrationEvent } from '../api/endpoints/integrations'
import { cardState, eventText, mcpCategory, serverState, stateDetail } from './integrations'

describe('mcpCategory', () => {
    it('keeps the server name unless a built-in category has it', () => {
        expect(mcpCategory('filesystem')).toBe('filesystem')
        expect(mcpCategory('memory')).toBe('memory_mcp')
        expect(mcpCategory('brave_search')).toBe('brave_search_mcp')
    })
})

describe('states', () => {
    it('is disabled whenever the switch is off', () => {
        expect(serverState({ state: 'running' }, false)).toBe('disabled')
        expect(cardState({ state: 'running' }, false)).toBe('disabled')
    })

    it('maps what the agent sees to the design states', () => {
        expect(serverState({ state: 'running', tools: 3 }, true)).toBe('connected')
        expect(serverState({ state: 'reconnecting', last_error: 'x' }, true)).toBe('reconnecting')
        expect(serverState({ state: 'unreachable', last_error: 'x' }, true)).toBe('failed')
        expect(cardState({ state: 'failed', last_error: 'bad token' }, true)).toBe('failed')
        expect(cardState({ state: 'running' }, true)).toBe('reading')
    })

    it('says why and when to expect a retry', () => {
        const now = Date.parse('2026-10-23T10:00:00Z')
        expect(stateDetail({ state: 'reconnecting', last_error: 'timed out', next_retry_at: '2026-10-23T10:00:20Z' }, now)).toBe('timed out · retry in 20s')
        expect(stateDetail({ state: 'incomplete', missing: ['command'] })).toBe('needs command')
        expect(stateDetail({ state: 'running' })).toBeUndefined()
    })
})

describe('eventText', () => {
    const event = (patch: Partial<IntegrationEvent>): IntegrationEvent => ({
        id: '1', integration_name: 'filesystem', family: 'mcp', action: 'created', fields: [], actor_name: 'Ana', at: '', ...patch,
    })

    it('reads as a sentence', () => {
        expect(eventText(event({}))).toBe('added filesystem')
        expect(eventText(event({ action: 'edited', fields: ['args'] }))).toBe('changed filesystem (args)')
        expect(eventText(event({ action: 'grant_on' }))).toBe('let the agent manage its own mcp integrations')
    })
})
