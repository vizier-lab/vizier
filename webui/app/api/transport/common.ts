// What both transports share: the error type, the saved token, and how a response becomes a result.

export class ApiError extends Error {
    constructor(
        public status: number,
        public code: string | undefined,
        message: string,
        public data?: unknown
    ) {
        super(message)
    }
}

export type Envelope<T> = { status: number; message?: string | null; data?: T | null; code?: string }

export type ConnectionState = 'connecting' | 'open' | 'reconnecting' | 'offline'

export interface TopicOptions {
    onFrame: (data: unknown) => void
    onState: (state: ConnectionState) => void
    /** Fired on every open; `reopened` is true after a drop, when history must be re-read. */
    onOpen: (reopened: boolean) => void
}

export interface TopicHandle {
    readonly isOpen: boolean
    send(frame: object): boolean
    retryNow(): void
    close(): void
}

export type AuthMode = 'http' | 'desktop'

export type DesktopState = {
    state: 'starting' | 'locked' | 'needs_owner' | 'choose_owner' | 'ready' | 'degraded' | 'failed'
    detail?: unknown
}

export interface DesktopShell {
    state(): Promise<DesktopState>
    onState(fn: (s: DesktopState) => void): Promise<() => void>
    onNavigate(fn: (href: string) => void): Promise<() => void>
    onWebAccess(fn: (s: { running: boolean; url?: string; error?: string }) => void): Promise<() => void>
    ready(): Promise<void>
    setupOwner(username: string, password?: string): Promise<void>
    chooseOwner(userId: string): Promise<void>
    openWorkspace(path?: string): Promise<void>
    restart(): Promise<void>
    settingsGet(): Promise<unknown>
    settingsSet(settings: unknown): Promise<unknown>
    updateCheck(): Promise<{ available: boolean; version?: string; notes?: string }>
    updateInstall(): Promise<void>
}

export interface RequestInit2 {
    signal?: AbortSignal
    raw?: boolean
}

export interface Transport {
    authMode: AuthMode
    request<T>(method: string, path: string, body?: unknown, init?: RequestInit2): Promise<T>
    upload<T>(path: string, form: FormData): Promise<T>
    fetchFile(path: string, init?: { range?: [number, number]; signal?: AbortSignal }): Promise<Response>
    assetUrl(path: string): string
    saveFile(path: string, filename: string): Promise<void>
    openFile(path: string, filename: string): Promise<void>
    openTopic(t: { agentId: string; channelId: string; topicId: string }, opts: TopicOptions): TopicHandle
    notify(n: { title: string; body: string; href: string }): Promise<void>
    desktop?: DesktopShell
}

export const TOKEN_KEY = 'auth_token'

export function getToken(): string | null {
    try {
        return localStorage.getItem(TOKEN_KEY)
    } catch {
        return null
    }
}

export function setToken(token: string | null) {
    try {
        if (token) localStorage.setItem(TOKEN_KEY, token)
        else localStorage.removeItem(TOKEN_KEY)
    } catch {
        /* storage blocked: the session lasts until reload */
    }
}

let onUnauthorized: () => void = () => {
    setToken(null)
    if (location.pathname !== '/login') location.assign('/login')
}

/** The session store installs its own handler so a 401 also clears the stored user. */
export function setUnauthorizedHandler(fn: () => void) {
    onUnauthorized = fn
}

/** Turns a status and parsed envelope into the result or the `ApiError`, the same for every transport. */
export function settle<T>(status: number, json: Envelope<T> | undefined | null, path: string, raw?: boolean): T {
    const body = json ?? undefined
    if (status === 401 && path !== '/auth/login') {
        onUnauthorized()
        throw new ApiError(401, 'unauthorized', body?.message ?? 'Signed out')
    }
    if (status < 200 || status >= 300) {
        const data = body?.data as { code?: string } | undefined
        throw new ApiError(status, body?.code ?? data?.code, body?.message ?? `HTTP ${status}`, body?.data)
    }
    return (raw ? (body as unknown as T) : (body?.data as T)) ?? (undefined as T)
}

export const API_BASE = '/api/v1'
