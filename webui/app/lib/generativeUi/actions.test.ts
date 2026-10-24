import { createStateStore } from '@json-render/core'
import { describe, expect, it, vi } from 'vitest'
import { ActionRefused, createActionHandlers } from './actions'

function setup(overrides: Partial<Parameters<typeof createActionHandlers>[0]> = {}) {
    let time = 10_000
    const store = createStateStore({ n: 1 })
    const sendEvent = vi.fn()
    const handlers = createActionHandlers({ store, sendEvent, isTrusted: () => true, now: () => time, ...overrides })
    return { handlers, sendEvent, store, advance: (ms: number) => (time += ms) }
}

describe('sendMessage', () => {
    it('sends after a trusted gesture', () => {
        const { handlers, sendEvent } = setup()
        handlers.sendMessage({ text: 'Your move', data: { board: [1] } })
        expect(sendEvent).toHaveBeenCalledWith('Your move', { board: [1] })
    })

    it('is refused when a timer, load or function started the chain', () => {
        const { handlers, sendEvent } = setup({ isTrusted: () => false })
        expect(() => handlers.sendMessage({ text: 'hi' })).toThrow(ActionRefused)
        expect(sendEvent).not.toHaveBeenCalled()
    })

    it('allows one a second', () => {
        const { handlers, sendEvent, advance } = setup()
        handlers.sendMessage({ text: 'a' })
        expect(() => handlers.sendMessage({ text: 'b' })).toThrow(/one a second/)
        advance(999)
        expect(() => handlers.sendMessage({ text: 'b' })).toThrow(/one a second/)
        advance(1)
        handlers.sendMessage({ text: 'c' })
        expect(sendEvent).toHaveBeenCalledTimes(2)
    })

    it('refuses empty, long and heavy messages without using up the interval', () => {
        const { handlers, sendEvent } = setup()
        expect(() => handlers.sendMessage({ text: '  ' })).toThrow(/needs text/)
        expect(() => handlers.sendMessage({ text: 'x'.repeat(4001) })).toThrow(/4000/)
        expect(() => handlers.sendMessage({ text: 'ok', data: 'x'.repeat(70 * 1024) })).toThrow(/64 KB/)
        handlers.sendMessage({ text: 'fine' })
        expect(sendEvent).toHaveBeenCalledTimes(1)
    })

    it('tells the page it was sent', () => {
        const onSent = vi.fn()
        setup({ onSent }).handlers.sendMessage({ text: 'hi' })
        expect(onSent).toHaveBeenCalled()
    })
})

describe('run', () => {
    it('writes the result into state', async () => {
        const runLogic = vi.fn().mockResolvedValue({ turn: 'b' })
        const { handlers, store } = setup({ runLogic })
        await handlers.run({ fn: 'step', args: [1, 2], into: '/game' })
        expect(runLogic).toHaveBeenCalledWith('step', [1, 2])
        expect(store.get('/game')).toEqual({ turn: 'b' })
    })

    it('discards the result without into', async () => {
        const { handlers, store } = setup({ runLogic: async () => 5 })
        await handlers.run({ fn: 'f' })
        expect(store.getSnapshot()).toEqual({ n: 1 })
    })

    it('leaves state alone, reports and rethrows on failure so the chain stops', async () => {
        const onError = vi.fn()
        const { handlers, store } = setup({ runLogic: async () => Promise.reject(new Error('boom')), onError })
        await expect(handlers.run({ fn: 'f', into: '/x' })).rejects.toThrow('boom')
        expect(store.get('/x')).toBeUndefined()
        expect(onError).toHaveBeenCalledWith('f', 'boom')
    })

    it('refuses when there is no logic', async () => {
        await expect(setup().handlers.run({ fn: 'f' })).rejects.toThrow(ActionRefused)
    })
})
