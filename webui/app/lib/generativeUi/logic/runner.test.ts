// The real runner script, run in a Node `vm` against a fake window: it is the text that ships.
import vm from 'node:vm'
import { describe, expect, it } from 'vitest'
import { MAX_CALL_DEPTH, RELAY_SOURCE, RUNNER_DOCUMENT, WORKER_SOURCE } from './runnerSource'

function boot() {
    const posted: any[] = []
    const listeners: ((e: any) => void)[] = []
    const self: any = { postMessage: (m: unknown) => posted.push(m), addEventListener: (_: string, l: (e: any) => void) => listeners.push(l), setTimeout: () => 1, fetch: () => 1, localStorage: {} }
    const context = vm.createContext({ self, Proxy, JSON, Object, Array, Function, String, Error, Math })
    vm.runInContext(WORKER_SOURCE, context)
    const send = (data: unknown) => listeners.forEach((l) => l({ data }))
    return { posted, window: self, send }
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

describe('the worker script', () => {
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

describe('the frame document', () => {
    it('opens with a CSP that allows no network, style or image, and only a blob worker', () => {
        const meta = RUNNER_DOCUMENT.indexOf('Content-Security-Policy')
        expect(meta).toBeGreaterThan(0)
        expect(meta).toBeLessThan(RUNNER_DOCUMENT.indexOf('<script>'))
        expect(RUNNER_DOCUMENT).toContain("default-src 'none'")
        expect(RUNNER_DOCUMENT).toContain("connect-src 'none'")
        expect(RUNNER_DOCUMENT).toContain("worker-src blob:")
        expect(RUNNER_DOCUMENT).toContain("script-src 'unsafe-eval' 'unsafe-inline' blob:")
    })

    it('embeds the worker without a way out of the script element', () => {
        expect(RELAY_SOURCE).not.toContain('</script')
        expect(RUNNER_DOCUMENT.match(/<\/script>/g)?.length).toBe(1)
    })

    it('relays only from its parent window', () => {
        const calls: unknown[] = []
        const listeners: ((e: { source: unknown; data: unknown }) => void)[] = []
        const parent = { postMessage: () => {} }
        const worker = { postMessage: (m: unknown) => calls.push(m), onmessage: null as unknown }
        const window = { parent, addEventListener: (_: string, l: (e: { source: unknown; data: unknown }) => void) => listeners.push(l) }
        const sandbox = { window, URL: { createObjectURL: () => 'blob:x' }, Blob: class {}, Worker: function () { return worker } }
        vm.runInContext(RELAY_SOURCE, vm.createContext(sandbox))
        listeners.forEach((l) => l({ source: { postMessage() {} }, data: { type: 'call', id: 1 } }))
        expect(calls).toEqual([])
        listeners.forEach((l) => l({ source: parent, data: { type: 'call', id: 2 } }))
        expect(calls).toEqual([{ type: 'call', id: 2 }])
    })
})
