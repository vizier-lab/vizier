import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeWorker } from './worker'

// A stand-in for the browser's Worker: runs the function it was given (the blob's text is not
// evaluated here), answering after `delay` ms of fake time.
type Reply = { ok: boolean; value?: unknown; error?: string }
const made: FakeWorker[] = []
let delay = 0
let answer: (input: unknown) => Reply = (input) => ({ ok: true, value: input })

class FakeWorker {
    onmessage: ((event: { data: Reply }) => void) | null = null
    onerror: ((event: { message: string; preventDefault(): void }) => void) | null = null
    terminated = false
    constructor() {
        made.push(this)
    }
    postMessage(input: unknown) {
        setTimeout(() => {
            if (!this.terminated) this.onmessage?.({ data: answer(input) })
        }, delay)
    }
    terminate() {
        this.terminated = true
    }
}

describe('vizier.worker', () => {
    beforeEach(() => {
        vi.useFakeTimers()
        vi.stubGlobal('Worker', FakeWorker)
        made.length = 0
        delay = 0
        answer = (input) => ({ ok: true, value: input })
    })
    afterEach(() => vi.useRealTimers())

    it('answers a run with the function result', async () => {
        const w = makeWorker((n: unknown) => n)
        const result = w.run(41)
        await vi.runAllTimersAsync()
        await expect(result).resolves.toBe(41)
    })

    it('terminates a run past its time limit and starts afresh for the next', async () => {
        const w = makeWorker((n: unknown) => n)
        delay = 5_000
        const slow = w.run(1, { timeoutMs: 100 })
        const caught = slow.catch((e: Error) => e.message)
        await vi.advanceTimersByTimeAsync(150)
        await expect(caught).resolves.toBe('timeout')
        expect(made[0].terminated).toBe(true)

        delay = 0
        const next = w.run(2)
        await vi.runAllTimersAsync()
        await expect(next).resolves.toBe(2)
        expect(made).toHaveLength(2)
    })

    it('runs one at a time, in order, and reports a thrown error', async () => {
        const w = makeWorker((n: unknown) => n)
        answer = (input) => (input === 'bad' ? { ok: false, error: 'nope' } : { ok: true, value: input })
        const order: unknown[] = []
        const a = w.run('a').then((v) => order.push(v))
        const bad = w.run('bad').catch((e: Error) => order.push(e.message))
        const c = w.run('c').then((v) => order.push(v))
        await vi.runAllTimersAsync()
        await Promise.all([a, bad, c])
        expect(order).toEqual(['a', 'nope', 'c'])
    })

    it('never waits longer than a minute, whatever it is asked', async () => {
        const w = makeWorker((n: unknown) => n)
        delay = 120_000
        const caught = w.run(1, { timeoutMs: 10 * 60_000 }).catch((e: Error) => e.message)
        await vi.advanceTimersByTimeAsync(60_001)
        await expect(caught).resolves.toBe('timeout')
    })
})
