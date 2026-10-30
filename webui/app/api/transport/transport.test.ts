import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// One fixture table drives both transports: the same result or error for the same server answer.
const cases: { name: string; status: number; body: unknown; expect: { ok: unknown } | { error: { status: number; code?: string } } }[] = [
    { name: 'success envelope', status: 200, body: { status: 200, data: { a: 1 } }, expect: { ok: { a: 1 } } },
    { name: 'error with top-level code', status: 409, body: { status: 409, code: 'token_in_use', message: 'used' }, expect: { error: { status: 409, code: 'token_in_use' } } },
    { name: 'error with data.code', status: 422, body: { status: 422, message: 'bad', data: { code: 'moved_to_integrations' } }, expect: { error: { status: 422, code: 'moved_to_integrations' } } },
]

const invoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
    invoke: (...a: unknown[]) => invoke(...a),
    Channel: class {
        onmessage: ((m: unknown) => void) | null = null
    },
}))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))
vi.mock('@tauri-apps/plugin-notification', () => ({ sendNotification: vi.fn() }))

import { http } from './http'
import { tauri } from './tauri'

async function outcome(run: () => Promise<unknown>) {
    try {
        return { ok: await run() }
    } catch (e) {
        const err = e as { status: number; code?: string }
        return { error: { status: err.status, code: err.code } }
    }
}

describe('transports agree', () => {
    beforeEach(() => {
        invoke.mockReset()
    })
    afterEach(() => vi.unstubAllGlobals())

    for (const c of cases) {
        it(c.name, async () => {
            vi.stubGlobal('fetch', async () => new Response(JSON.stringify(c.body), { status: c.status }))
            invoke.mockResolvedValue({ status: c.status, body: c.body })
            const viaHttp = await outcome(() => http.request('GET', '/x'))
            const viaTauri = await outcome(() => tauri.request('GET', '/x'))
            expect(viaHttp).toEqual(viaTauri)
            if ('ok' in c.expect) expect(viaTauri).toEqual({ ok: c.expect.ok })
            else expect((viaTauri as { error: { status: number; code?: string } }).error).toEqual(expect.objectContaining(c.expect.error))
        })
    }

    it('raw returns the envelope on both', async () => {
        const body = { status: 200, data: 1, message: 'm' }
        vi.stubGlobal('fetch', async () => new Response(JSON.stringify(body)))
        invoke.mockResolvedValue({ status: 200, body })
        expect(await http.request('GET', '/x', undefined, { raw: true })).toEqual(body)
        expect(await tauri.request('GET', '/x', undefined, { raw: true })).toEqual(body)
    })

    it('tauri sends method, path and content type as invoke headers', async () => {
        invoke.mockResolvedValue({ status: 200, body: { status: 200, data: null } })
        await tauri.request('POST', '/agents', { a: 1 })
        const [cmd, bytes, opts] = invoke.mock.calls[0]
        expect(cmd).toBe('api')
        expect(new TextDecoder().decode(bytes as Uint8Array)).toBe('{"a":1}')
        expect(opts).toEqual({ headers: { 'x-method': 'POST', 'x-path': '/agents', 'content-type': 'application/json', 'x-content-type': 'application/json' } })
    })

    it('upload sends the multipart bytes with their boundary content type', async () => {
        invoke.mockResolvedValue({ status: 200, body: { status: 200, data: 'ok' } })
        const form = new FormData()
        form.append('file', new Blob(['hi']), 'a.txt')
        await tauri.upload('/files', form)
        const [, bytes, opts] = invoke.mock.calls[0]
        expect((bytes as ArrayBuffer).byteLength).toBeGreaterThan(2)
        expect(opts.headers['content-type']).toMatch(/^multipart\/form-data; boundary=/)
    })

    it('assetUrl shapes', () => {
        expect(http.assetUrl('/files/a')).toBe('/api/v1/files/a')
        expect(tauri.assetUrl('/files/a')).toBe('vizier-asset://localhost/api/v1/files/a')
    })

    it('tauri topics open, send and close', async () => {
        invoke.mockImplementation(async (cmd: string) => (cmd === 'topic_open' ? { id: 7 } : undefined))
        const states: string[] = []
        const opened: boolean[] = []
        const h = tauri.openTopic({ agentId: 'a', channelId: 'c', topicId: 't' }, { onFrame() {}, onState: (s) => states.push(s), onOpen: (r) => opened.push(r) })
        await vi.waitFor(() => expect(h.isOpen).toBe(true))
        expect(opened).toEqual([false])
        expect(h.send({ x: 1 })).toBe(true)
        expect(invoke).toHaveBeenCalledWith('topic_send', { id: 7, frame: { x: 1 } })
        h.close()
        expect(invoke).toHaveBeenCalledWith('topic_close', { id: 7 })
        expect(h.isOpen).toBe(false)
    })
})
