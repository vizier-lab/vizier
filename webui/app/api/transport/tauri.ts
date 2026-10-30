// The desktop transport: every call goes to the in-process engine through Tauri commands.
import { Channel, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { sendNotification } from '@tauri-apps/plugin-notification'
import { API_BASE, relativeToApi, settle, type DesktopShell, type DesktopState, type Envelope, type Transport, type UpdateInfo, type TopicHandle, type TopicOptions } from './common'

const ASSET = 'vizier-asset://localhost'
const encoder = new TextEncoder()

type ApiResult<T> = { status: number; body: Envelope<T> | null }

async function call<T>(method: string, path: string, bytes: Uint8Array | ArrayBuffer | undefined, contentType: string | undefined, raw?: boolean): Promise<T> {
    const headers: Record<string, string> = { 'x-method': method, 'x-path': path }
    if (contentType) {
        headers['content-type'] = contentType
        // `invoke` merges its own octet-stream label with ours; the shell reads this one first.
        headers['x-content-type'] = contentType
    }
    const res = await invoke<ApiResult<T>>('api', bytes ?? new Uint8Array(), { headers })
    return settle<T>(res.status, res.body, path, raw)
}

export const tauriShell: DesktopShell = {
    state: () => invoke<DesktopState>('desktop_state'),
    onState: (fn) => listen<DesktopState>('desktop://state', (e) => fn(e.payload)),
    onNavigate: (fn) => listen<{ href: string }>('desktop://navigate', (e) => fn(e.payload.href)),
    onWebAccess: (fn) => listen<{ running: boolean; url?: string; error?: string }>('desktop://web-access', (e) => fn(e.payload)),
    ready: () => invoke('desktop_ready'),
    setupOwner: (username, password) => invoke('desktop_setup_owner', { username, password }),
    chooseOwner: (userId) => invoke('desktop_choose_owner', { userId }),
    openWorkspace: (path) => invoke('desktop_open_workspace', { path }),
    restart: () => invoke('runtime_restart'),
    settingsGet: () => invoke('desktop_settings_get'),
    settingsSet: (settings) => invoke('desktop_settings_set', { patch: settings }),
    onUpdate: (fn) => listen<UpdateInfo>('desktop://update', (e) => fn(e.payload)),
    updateCheck: () => invoke('update_check'),
    updateInstall: () => invoke('update_install'),
}

export const tauri: Transport = {
    authMode: 'desktop',
    request: (method, path, body, init) =>
        call(method, path, body === undefined ? undefined : encoder.encode(JSON.stringify(body)), body === undefined ? undefined : 'application/json', init?.raw),
    async upload(path, form) {
        const res = new Response(form)
        return call('POST', path, await res.arrayBuffer(), res.headers.get('content-type') ?? undefined)
    },
    fetchFile(path, init) {
        const headers: Record<string, string> = {}
        if (init?.range) headers.Range = `bytes=${init.range[0]}-${init.range[1]}`
        return fetch(`${ASSET}${API_BASE}${path}`, { headers, signal: init?.signal })
    },
    assetUrl: (path) => `${ASSET}${API_BASE}${relativeToApi(path)}`,
    saveFile: (path, filename) => invoke('save_file', { path: `${API_BASE}${path}`, filename }),
    openFile: (path, filename) => invoke('open_file', { path: `${API_BASE}${path}`, filename }),
    openTopic(t, opts: TopicOptions): TopicHandle {
        let id: number | null = null
        let closed = false
        let everOpened = false
        let unlisten: (() => void) | null = null

        const open = async () => {
            if (closed) return
            opts.onState(everOpened ? 'reconnecting' : 'connecting')
            const frames = new Channel<unknown>()
            frames.onmessage = (frame) => opts.onFrame(frame)
            try {
                const res = await invoke<{ id: number }>('topic_open', { agentId: t.agentId, channelId: t.channelId, topicId: t.topicId, frames })
                if (closed) {
                    void invoke('topic_close', { id: res.id })
                    return
                }
                id = res.id
                const reopened = everOpened
                everOpened = true
                opts.onState('open')
                opts.onOpen(reopened)
            } catch {
                id = null
                opts.onState('offline')
            }
        }

        void open()
        // The engine restarting drops its topics; open again once it is ready.
        void listen<DesktopState>('desktop://state', (e) => {
            if (e.payload.state === 'ready' && everOpened && !closed) {
                id = null
                void open()
            }
        }).then((un) => (closed ? un() : (unlisten = un)))

        return {
            get isOpen() {
                return id !== null
            },
            send(frame) {
                if (id === null) return false
                void invoke('topic_send', { id, frame })
                return true
            },
            retryNow() {
                if (id === null) void open()
            },
            close() {
                closed = true
                unlisten?.()
                if (id !== null) void invoke('topic_close', { id })
                id = null
            },
        }
    },
    async notify({ title, body, href }) {
        await invoke('notify', { title, body, href }).catch(() => sendNotification({ title, body }))
    },
    desktop: tauriShell,
}
