import { transport } from './transport'
export { ApiError, TOKEN_KEY, API_BASE, getToken, setToken, setUnauthorizedHandler } from './transport/common'

/** Sends a request and unwraps the `APIResponse` envelope. */
export function request<T>(method: string, path: string, body?: unknown, init?: { signal?: AbortSignal; raw?: boolean }): Promise<T> {
    if (typeof FormData !== 'undefined' && body instanceof FormData) return transport.upload<T>(path, body)
    return transport.request<T>(method, path, body, init)
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
