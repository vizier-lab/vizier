import { createStateStore } from '@json-render/core'
import { describe, expect, it, vi } from 'vitest'
import { currentChainIsTrusted, noteGesture } from './trust'
import { collectWatches, startWatching } from './watch'

const flush = () => new Promise((r) => setTimeout(r, 0))

function harness() {
    const store = createStateStore({ seed: 1, out: 0 })
    const listeners = new Set<() => void>()
    const subscribe = (l: () => void) => (listeners.add(l), () => listeners.delete(l))
    const set = (path: string, value: unknown) => {
        store.set(path, value)
        listeners.forEach((l) => l())
    }
    return { store, subscribe, set }
}

describe('collectWatches', () => {
    it('gathers bindings from every element, one or many', () => {
        const watches = collectWatches({
            elements: {
                a: { watch: { '/seed': { action: 'run', params: { fn: 'f' } } } },
                b: { watch: { '/x': [{ action: 'setState' }, { action: 'run' }] } },
                c: {},
            },
        })
        expect(watches.map((w) => [w.path, w.bindings.length])).toEqual([['/seed', 1], ['/x', 2]])
    })
})

describe('startWatching', () => {
    it('runs a binding when its path changes, untrusted, with params resolved from state', async () => {
        const { store, subscribe, set } = harness()
        const seen: unknown[] = []
        const execute = vi.fn(async (b) => {
            seen.push([b.params, currentChainIsTrusted()])
        })
        noteGesture(true)
        startWatching([{ path: '/seed', bindings: [{ action: 'run', params: { n: { $state: '/seed' } } }] }], store, subscribe, execute)
        set('/seed', 5)
        await flush()
        expect(seen).toEqual([[{ n: 5 }, false]])
    })

    it('ignores changes to other paths and does not fire at start', async () => {
        const { store, subscribe, set } = harness()
        const execute = vi.fn(async () => {})
        startWatching([{ path: '/seed', bindings: [{ action: 'x' }] }], store, subscribe, execute)
        set('/out', 3)
        await flush()
        expect(execute).not.toHaveBeenCalled()
    })

    it('does not start another chain from its own writes', async () => {
        const { store, subscribe, set } = harness()
        let runs = 0
        const execute = vi.fn(async () => {
            runs++
            set('/seed', 100 + runs)
        })
        startWatching([{ path: '/seed', bindings: [{ action: 'x' }] }], store, subscribe, execute)
        set('/seed', 2)
        await flush()
        await flush()
        expect(runs).toBe(1)
        set('/seed', 3)
        await flush()
        expect(runs).toBe(2)
    })

    it('stops a chain at the first failure and caps its length', async () => {
        const { store, subscribe, set } = harness()
        const calls: string[] = []
        const execute = vi.fn(async (b) => {
            calls.push(b.action)
            if (b.action === 'bad') throw new Error('no')
        })
        startWatching([{ path: '/seed', bindings: [{ action: 'a' }, { action: 'bad' }, { action: 'never' }] }], store, subscribe, execute)
        set('/seed', 2)
        await flush()
        expect(calls).toEqual(['a', 'bad'])

        const many = Array.from({ length: 12 }, (_, i) => ({ action: `s${i}` }))
        const { store: s2, subscribe: sub2, set: set2 } = harness()
        const ran: string[] = []
        startWatching([{ path: '/seed', bindings: many }], s2, sub2, async (b) => void ran.push(b.action))
        set2('/seed', 9)
        await flush()
        expect(ran.length).toBe(8)
    })

    it('stops when asked', async () => {
        const { store, subscribe, set } = harness()
        const execute = vi.fn(async () => {})
        const stop = startWatching([{ path: '/seed', bindings: [{ action: 'x' }] }], store, subscribe, execute)
        stop()
        set('/seed', 2)
        await flush()
        expect(execute).not.toHaveBeenCalled()
    })
})
