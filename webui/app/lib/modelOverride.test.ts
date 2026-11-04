import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { modelOverridePayload, readModelOverride, writeModelOverride, type ModelOverridePreferenceKey } from './modelOverride'

const stored = new Map<string, string>()
const storage = {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => { stored.set(key, value) },
    removeItem: (key: string) => { stored.delete(key) },
    clear: () => stored.clear(),
}
Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: storage })

afterEach(() => vi.restoreAllMocks())

const key: ModelOverridePreferenceKey = { userId: 'user.1', agentId: 'agent', channelId: 'vizier-webui', topicId: 'topic/a' }

beforeEach(() => storage.clear())

describe('model override preference', () => {
    it('reads and writes a complete model id and Agent default', () => {
        const choice = { provider: 'opencode', model: 'deepseek/deepseek-flash' }
        expect(readModelOverride(key)).toBeNull()
        writeModelOverride(key, choice)
        expect(readModelOverride(key)).toEqual(choice)
        writeModelOverride(key, null)
        expect(readModelOverride(key)).toBeNull()
    })

    it('builds a request payload without splitting or normalizing the model id', () => {
        expect(modelOverridePayload({ provider: 'opencode', model: 'deepseek/deepseek-flash' })).toEqual({
            model_override: { provider: 'opencode', model: 'deepseek/deepseek-flash' },
        })
        expect(modelOverridePayload(null)).toEqual({})
    })

    it('isolates users, channels and topics', () => {
        writeModelOverride(key, { provider: 'openai', model: 'gpt-luna' })
        expect(readModelOverride({ ...key, userId: 'other' })).toBeNull()
        expect(readModelOverride({ ...key, channelId: 'project__p1' })).toBeNull()
        expect(readModelOverride({ ...key, topicId: 'other' })).toBeNull()
    })

    it('treats malformed stored values and unavailable storage as Agent default', () => {
        storage.setItem('vizier.chat.model.user%2E1.agent.vizier-webui.topic%2Fa', '{')
        expect(readModelOverride(key)).toBeNull()
        vi.spyOn(storage, 'getItem').mockImplementation(() => { throw new Error('blocked') })
        expect(readModelOverride(key)).toBeNull()
    })
})
