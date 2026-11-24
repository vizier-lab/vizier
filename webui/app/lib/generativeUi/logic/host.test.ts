import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CALL_TIMEOUT_MS, LogicError, LogicHost, MAX_QUEUE, type FromRunner, type Transport } from './host'

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

/** A runner that answers by a script, so the host's queue and limits can be watched. */
class Fake implements Transport {
    static all: Fake[] = []
    sent: any[] = []
    destroyed = false
    private listener: (m: FromRunner) => void = () => {}
    constructor() {
        Fake.all.push(this)
    }
    send(message: unknown) {
        this.sent.push(message)
    }
    onMessage(l: (m: FromRunner) => void) {
        this.listener = l
    }
    destroy() {
        this.destroyed = true
    }
    hello() {
        this.listener({ type: 'hello' })
    }
    answer(id: number, value: unknown) {
        this.listener({ type: 'result', id, ok: true, json: JSON.stringify(value) })
    }
    fail(id: number, error: string) {
        this.listener({ type: 'result', id, ok: false, error })
    }
    calls() {
        return this.sent.filter((m) => m.type === 'call')
    }
}

const make = () => {
    Fake.all = []
    return new LogicHost({ f: { params: ['a'], body: 'return a' } }, () => new Fake())
}

describe('LogicHost', () => {
    it('starts on the first call, sends the definitions once ready, and resolves the result', async () => {
        const host = make()
        const result = host.call('f', [1])
        expect(Fake.all.length).toBe(1)
        expect(Fake.all[0].sent).toEqual([])
        Fake.all[0].hello()
        expect(Fake.all[0].sent[0]).toEqual({ type: 'init', defs: { f: { params: ['a'], body: 'return a' } } })
        const [call] = Fake.all[0].calls()
        expect(call).toMatchObject({ fn: 'f', args: [1] })
        Fake.all[0].answer(call.id, { n: 2 })
        await expect(result).resolves.toEqual({ n: 2 })
    })

    it('runs one call at a time, in order', async () => {
        const host = make()
        const a = host.call('f', ['a'])
        const b = host.call('f', ['b'])
        const t = Fake.all[0]
        t.hello()
        expect(t.calls().length).toBe(1)
        t.answer(t.calls()[0].id, 'A')
        await a
        expect(t.calls().length).toBe(2)
        t.answer(t.calls()[1].id, 'B')
        await expect(b).resolves.toBe('B')
    })

    it('rejects a call that runs over 250 ms, destroys the frame and carries on in a new one', async () => {
        const host = make()
        const slow = host.call('f', [1])
        const next = host.call('f', [2])
        Fake.all[0].hello()
        const caught = expect(slow).rejects.toMatchObject({ code: 'logic_timeout' })
        vi.advanceTimersByTime(CALL_TIMEOUT_MS)
        await caught
        expect(Fake.all[0].destroyed).toBe(true)
        expect(Fake.all.length).toBe(2)
        Fake.all[1].hello()
        Fake.all[1].answer(Fake.all[1].calls()[0].id, 'ok')
        await expect(next).resolves.toBe('ok')
    })

    it('does not time a call out once it has answered', async () => {
        const host = make()
        const p = host.call('f', [1])
        Fake.all[0].hello()
        Fake.all[0].answer(Fake.all[0].calls()[0].id, 1)
        await p
        vi.advanceTimersByTime(1000)
        expect(Fake.all[0].destroyed).toBe(false)
    })

    it('refuses more than 16 waiting calls', async () => {
        const host = make()
        const waiting = Array.from({ length: MAX_QUEUE + 1 }, () => host.call('f', [0]))
        waiting.slice(0, 2).forEach((p) => p.catch(() => {}))
        await expect(host.call('f', [0])).rejects.toMatchObject({ code: 'logic_busy' })
        host.dispose()
        await Promise.allSettled(waiting)
    })

    it('rejects oversize arguments and results', async () => {
        const host = make()
        await expect(host.call('f', ['x'.repeat(300 * 1024)])).rejects.toMatchObject({ code: 'logic_args_too_large' })
        const p = host.call('f', [1])
        Fake.all[0].hello()
        Fake.all[0].answer(Fake.all[0].calls()[0].id, 'x'.repeat(300 * 1024))
        await expect(p).rejects.toMatchObject({ code: 'logic_result_too_large' })
    })

    it('turns a thrown error into a rejection with the message', async () => {
        const host = make()
        const p = host.call('f', [1])
        Fake.all[0].hello()
        Fake.all[0].fail(Fake.all[0].calls()[0].id, 'boom')
        const error = await p.catch((e) => e)
        expect(error).toBeInstanceOf(LogicError)
        expect(error).toMatchObject({ code: 'logic_error', message: 'boom' })
    })

    it('ignores answers for calls it is not waiting on', async () => {
        const host = make()
        const p = host.call('f', [1])
        Fake.all[0].hello()
        Fake.all[0].answer(999, 'stray')
        Fake.all[0].answer(Fake.all[0].calls()[0].id, 'real')
        await expect(p).resolves.toBe('real')
    })

    it('rejects pending calls on dispose and refuses new ones', async () => {
        const host = make()
        const p = host.call('f', [1])
        host.dispose()
        await expect(p).rejects.toMatchObject({ code: 'logic_disposed' })
        await expect(host.call('f', [1])).rejects.toMatchObject({ code: 'logic_disposed' })
        expect(Fake.all[0].destroyed).toBe(true)
    })

    it('releases an idle runner and starts another on demand, but never one with work', async () => {
        const host = make()
        const p = host.call('f', [1])
        Fake.all[0].hello()
        host.release()
        expect(Fake.all[0].destroyed).toBe(false)
        Fake.all[0].answer(Fake.all[0].calls()[0].id, 1)
        await p
        host.release()
        expect(Fake.all[0].destroyed).toBe(true)
        const again = host.call('f', [2])
        expect(Fake.all.length).toBe(2)
        Fake.all[1].hello()
        Fake.all[1].answer(Fake.all[1].calls()[0].id, 2)
        await expect(again).resolves.toBe(2)
    })
})
