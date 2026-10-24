// The real runner script, run in a Node `vm` against a fake window: it is the text that ships.
import vm from 'node:vm'
import { describe, expect, it } from 'vitest'
import { MAX_CALL_DEPTH, RUNNER_DOCUMENT, RUNNER_SOURCE } from './runnerSource'

function boot() {
    const posted: any[] = []
    const listeners: ((e: any) => void)[] = []
    const parent = { postMessage: (m: unknown) => posted.push(m) }
    const window: any = { parent, addEventListener: (_: string, l: (e: any) => void) => listeners.push(l), setTimeout: () => 1, fetch: () => 1, localStorage: {} }
    const context = vm.createContext({ window, Proxy, JSON, Object, Array, Function, String, Error, Math })
    vm.runInContext(RUNNER_SOURCE, context)
    const send = (data: unknown, source: unknown = parent) => listeners.forEach((l) => l({ source, data }))
    return { posted, window, send, parent }
}

const defs = {
    add: { params: ['a', 'b'], body: 'return a + b' },
    twice: { params: ['n'], body: 'return fns.add(n, n)' },
    loopless: { params: [], body: 'return undefined' },
    boom: { params: [], body: 'throw new Error("nope")' },
    deep: { params: ['n'], body: 'return fns.deep(n + 1)' },
    bad: { params: [], body: 'return (' },
    cyc: { params: [], body: 'var o = {}; o.o = o; return o' },
    net: { params: [], body: 'return [typeof fetch, typeof setTimeout, typeof localStorage]' },
}

const results = (posted: any[]) => posted.filter((m) => m.type === 'result')

describe('the runner script', () => {
    it('says hello and removes what a function could reach for', () => {
        const { posted, window } = boot()
        expect(posted[0]).toEqual({ type: 'hello' })
        for (const name of ['setTimeout', 'fetch', 'localStorage']) expect(window[name]).toBeUndefined()
    })

    it('calls a function and answers with JSON', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs })
        send({ type: 'call', id: 1, fn: 'add', args: [2, 3] })
        expect(results(posted)[0]).toEqual({ type: 'result', id: 1, ok: true, json: '5' })
    })

    it('lets functions call each other through fns', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs })
        send({ type: 'call', id: 1, fn: 'twice', args: [4] })
        expect(results(posted)[0].json).toBe('8')
    })

    it('answers undefined as null', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs })
        send({ type: 'call', id: 1, fn: 'loopless', args: [] })
        expect(results(posted)[0].json).toBe('null')
    })

    it('reports a thrown error, a missing function, a syntax error and a non-JSON result by message', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs })
        for (const [id, fn] of [[1, 'boom'], [2, 'missing'], [3, 'bad'], [4, 'cyc']] as const) send({ type: 'call', id, fn, args: [] })
        const [boom, missing, bad, cyc] = results(posted)
        expect(boom).toMatchObject({ ok: false, error: 'nope' })
        expect(missing).toMatchObject({ ok: false, error: 'no function named missing' })
        expect(bad.ok).toBe(false)
        expect(cyc.ok).toBe(false)
    })

    it('stops runaway recursion at the call depth limit and keeps working', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs })
        send({ type: 'call', id: 1, fn: 'deep', args: [0] })
        expect(results(posted)[0]).toMatchObject({ ok: false, error: 'call depth exceeded' })
        send({ type: 'call', id: 2, fn: 'add', args: [1, 1] })
        expect(results(posted)[1].json).toBe('2')
        expect(MAX_CALL_DEPTH).toBe(32)
    })

    it('believes only its parent', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs })
        send({ type: 'call', id: 1, fn: 'add', args: [1, 2] }, { postMessage() {} })
        expect(results(posted)).toEqual([])
    })

    it('ignores messages that are not calls', () => {
        const { posted, send } = boot()
        send(null)
        send('hi')
        send({ type: 'call', id: 'x', fn: 'add' })
        expect(results(posted)).toEqual([])
    })

    it('truncates long error messages', () => {
        const { posted, send } = boot()
        send({ type: 'init', defs: { long: { params: [], body: 'throw new Error("x".repeat(1000))' } } })
        send({ type: 'call', id: 1, fn: 'long', args: [] })
        expect(results(posted)[0].error.length).toBe(300)
    })
})

describe('the runner document', () => {
    it('opens with a CSP that allows no network, style or image', () => {
        const meta = RUNNER_DOCUMENT.indexOf('Content-Security-Policy')
        expect(meta).toBeGreaterThan(0)
        expect(meta).toBeLessThan(RUNNER_DOCUMENT.indexOf('<script>'))
        expect(RUNNER_DOCUMENT).toContain("default-src 'none'")
        expect(RUNNER_DOCUMENT).toContain("connect-src 'none'")
        expect(RUNNER_DOCUMENT).toContain("script-src 'unsafe-eval' 'unsafe-inline'")
    })
})
