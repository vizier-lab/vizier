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

type Envelope<T> = { status: number; message?: string | null; data?: T | null }

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

export const API_BASE = '/api/v1'

let onUnauthorized: () => void = () => {
    setToken(null)
    if (location.pathname !== '/login') location.assign('/login')
}

/** The session store installs its own handler so a 401 also clears the stored user. */
export function setUnauthorizedHandler(fn: () => void) {
    onUnauthorized = fn
}

/** Sends a request and unwraps the `APIResponse` envelope. */
export async function request<T>(
    method: string,
    path: string,
    body?: unknown,
    init?: { signal?: AbortSignal; raw?: boolean }
): Promise<T> {
    const headers: Record<string, string> = {}
    const token = getToken()
    if (token) headers.Authorization = `Bearer ${token}`
    const isForm = typeof FormData !== 'undefined' && body instanceof FormData
    if (body !== undefined && !isForm) headers['Content-Type'] = 'application/json'

    const res = await fetch(`${API_BASE}${path}`, {
        method,
        headers,
        body:
            body === undefined
                ? undefined
                : isForm
                  ? (body as FormData)
                  : JSON.stringify(body),
        signal: init?.signal,
    })

    let json: (Envelope<T> & { code?: string }) | undefined
    try {
        json = (await res.json()) as Envelope<T> & { code?: string }
    } catch {
        json = undefined
    }

    if (res.status === 401 && path !== '/auth/login') {
        onUnauthorized()
        throw new ApiError(401, 'unauthorized', json?.message ?? 'Signed out')
    }
    if (!res.ok) {
        const data = json?.data as { code?: string } | undefined
        throw new ApiError(
            res.status,
            json?.code ?? data?.code,
            json?.message ?? res.statusText,
            json?.data
        )
    }
    return (init?.raw ? (json as unknown as T) : (json?.data as T)) ?? (undefined as T)
}

export const api = {
    get: <T>(path: string, signal?: AbortSignal) => request<T>('GET', path, undefined, { signal }),
    post: <T>(path: string, body?: unknown) => request<T>('POST', path, body),
    put: <T>(path: string, body?: unknown) => request<T>('PUT', path, body),
    patch: <T>(path: string, body?: unknown) => request<T>('PATCH', path, body),
    delete: <T>(path: string) => request<T>('DELETE', path),
}

export type SortOrder = 'asc' | 'desc'

/** The opt-in list query (spec 024, R7). `limit` present switches the server to a `Page`. */
export type ListQuery = {
    q?: string
    filter?: string
    sort?: string
    order?: SortOrder
    offset?: number
    limit?: number
}

export type Page<T> = {
    items: T[]
    total: number
    counts: Record<string, number>
}

export function queryString(params: Record<string, string | number | undefined | null>): string {
    const qs = new URLSearchParams()
    for (const [k, v] of Object.entries(params)) {
        if (v !== undefined && v !== null && v !== '') qs.set(k, String(v))
    }
    const s = qs.toString()
    return s ? `?${s}` : ''
}

/** `GET path?q&filter&sort&order&offset&limit`, answered as a `Page`. */
export function paged<T>(path: string, query: ListQuery, signal?: AbortSignal) {
    const sep = path.includes('?') ? '&' : '?'
    return api.get<Page<T>>(`${path}${sep}${queryString({ ...query }).slice(1)}`, signal)
}
