import type { ListQuery, SortOrder } from '../api/client'

export const PAGE_SIZE = 25

export type ListState = {
    q: string
    filter: string
    sort: string
    order?: SortOrder
    page: number
}

/** Reads the list state out of a URL query, falling back to the screen's defaults. */
export function parseListState(params: URLSearchParams, defaults: Partial<ListState> = {}): ListState {
    const page = Number(params.get('page'))
    const order = params.get('order')
    return {
        q: params.get('q') ?? defaults.q ?? '',
        filter: params.get('filter') ?? defaults.filter ?? 'all',
        sort: params.get('sort') ?? defaults.sort ?? '',
        order: order === 'asc' || order === 'desc' ? order : defaults.order,
        page: Number.isInteger(page) && page >= 1 ? page : 1,
    }
}

/** Writes the list state back, leaving out whatever equals its default so URLs stay short. */
export function writeListState(
    params: URLSearchParams,
    state: ListState,
    defaults: Partial<ListState> = {}
): URLSearchParams {
    const next = new URLSearchParams(params)
    const set = (key: string, value: string | undefined, def: string | undefined) => {
        if (value && value !== def) next.set(key, value)
        else next.delete(key)
    }
    set('q', state.q, defaults.q ?? '')
    set('filter', state.filter, defaults.filter ?? 'all')
    set('sort', state.sort, defaults.sort ?? '')
    set('order', state.order, defaults.order)
    set('page', state.page > 1 ? String(state.page) : undefined, undefined)
    return next
}

export function toListQuery(state: ListState, perPage = PAGE_SIZE): ListQuery {
    return {
        q: state.q.trim() || undefined,
        filter: state.filter === 'all' ? undefined : state.filter,
        sort: state.sort || undefined,
        order: state.order,
        offset: (state.page - 1) * perPage,
        limit: perPage,
    }
}

export function pageCount(total: number, perPage = PAGE_SIZE): number {
    return Math.max(1, Math.ceil(total / perPage))
}

/** "1–25 of 312" */
export function rangeLabel(page: number, total: number, perPage = PAGE_SIZE): string {
    if (total === 0) return '0 of 0'
    const from = (page - 1) * perPage + 1
    const to = Math.min(total, page * perPage)
    const f = (n: number) => n.toLocaleString('en-US')
    return `${f(from)}–${f(to)} of ${f(total)}`
}
