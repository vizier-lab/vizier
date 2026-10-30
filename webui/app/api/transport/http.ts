// The browser transport: fetch and a WebSocket per topic, with a bearer token.
import { API_BASE, getToken, relativeToApi, settle, type Envelope, type Transport, type TopicHandle, type TopicOptions } from './common'
import { TopicSocket } from './websocket'

async function send<T>(method: string, path: string, body: unknown, init?: { signal?: AbortSignal; raw?: boolean }): Promise<T> {
    const headers: Record<string, string> = {}
    const token = getToken()
    if (token) headers.Authorization = `Bearer ${token}`
    const isForm = typeof FormData !== 'undefined' && body instanceof FormData
    if (body !== undefined && !isForm) headers['Content-Type'] = 'application/json'
    const res = await fetch(`${API_BASE}${path}`, {
        method,
        headers,
        body: body === undefined ? undefined : isForm ? (body as FormData) : JSON.stringify(body),
        signal: init?.signal,
    })
    let json: Envelope<T> | undefined
    try {
        json = (await res.json()) as Envelope<T>
    } catch {
        json = undefined
    }
    if (!res.ok && json?.message == null && res.statusText) json = { ...(json ?? { status: res.status }), message: res.statusText }
    return settle<T>(res.status, json, path, init?.raw)
}

export function topicUrl(t: { agentId: string; channelId: string; topicId: string }, token: string): string {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    return `${proto}//${location.host}${API_BASE}/agents/${t.agentId}/channel/${encodeURIComponent(t.channelId)}/topic/${encodeURIComponent(t.topicId)}/chat?token=${encodeURIComponent(token)}`
}

export const http: Transport = {
    authMode: 'http',
    request: (method, path, body, init) => send(method, path, body, init),
    upload: (path, form) => send('POST', path, form),
    fetchFile(path, init) {
        const headers: Record<string, string> = {}
        const token = getToken()
        if (token) headers.Authorization = `Bearer ${token}`
        if (init?.range) headers.Range = `bytes=${init.range[0]}-${init.range[1]}`
        return fetch(`${API_BASE}${path}`, { headers, signal: init?.signal })
    },
    assetUrl(path) {
        path = relativeToApi(path)
        const token = getToken()
        const sep = path.includes('?') ? '&' : '?'
        return `${API_BASE}${path}${token ? `${sep}token=${encodeURIComponent(token)}` : ''}`
    },
    async saveFile(path, filename) {
        const res = await this.fetchFile(path)
        const href = URL.createObjectURL(await res.blob())
        const a = document.createElement('a')
        a.href = href
        a.download = filename
        a.click()
        URL.revokeObjectURL(href)
    },
    async openFile(path) {
        window.open(this.assetUrl(path), '_blank', 'noopener')
    },
    openTopic(t, opts: TopicOptions): TopicHandle {
        return new TopicSocket({
            ...opts,
            url: () => {
                const token = getToken()
                return token ? topicUrl(t, token) : null
            },
        })
    },
    async notify() {},
}
