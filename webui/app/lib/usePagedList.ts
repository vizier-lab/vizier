import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useSearchParams } from 'react-router'
import { paged, type Page } from '../api/client'
import {
    PAGE_SIZE,
    pageCount,
    parseListState,
    toListQuery,
    writeListState,
    type ListState,
} from './paging'

/**
 * A server-paged list whose `q`/`filter`/`sort`/`page` live in the URL, so a reload or a
 * shared link lands on the same page. Search is debounced; the other changes fetch at once.
 */
export function usePagedList<T>(
    path: string | null,
    defaults: Partial<ListState> = {},
    perPage = PAGE_SIZE
) {
    const [params, setParams] = useSearchParams()
    const state = useMemo(() => parseListState(params, defaults), [params]) // eslint-disable-line react-hooks/exhaustive-deps
    const [search, setSearch] = useState(state.q)
    const [data, setData] = useState<Page<T> | null>(null)
    const [error, setError] = useState<Error | null>(null)
    const [loading, setLoading] = useState(false)
    const [reloads, setReloads] = useState(0)

    const update = useCallback(
        (patch: Partial<ListState>) => {
            setParams(
                (prev) => {
                    const cur = parseListState(prev, defaults)
                    const resetsPage = 'q' in patch || 'filter' in patch || 'sort' in patch || 'order' in patch
                    return writeListState(prev, { ...cur, page: resetsPage ? 1 : cur.page, ...patch }, defaults)
                },
                { replace: true }
            )
        },
        [setParams] // eslint-disable-line react-hooks/exhaustive-deps
    )

    // Debounce typing into the URL.
    const first = useRef(true)
    useEffect(() => {
        if (first.current) {
            first.current = false
            return
        }
        if (search === state.q) return
        const t = setTimeout(() => update({ q: search }), 250)
        return () => clearTimeout(t)
    }, [search]) // eslint-disable-line react-hooks/exhaustive-deps

    const key = JSON.stringify([path, toListQuery(state, perPage), reloads])
    useEffect(() => {
        if (!path) return
        const ctl = new AbortController()
        setLoading(true)
        paged<T>(path, toListQuery(state, perPage), ctl.signal)
            .then((p) => {
                setData(p)
                setError(null)
            })
            .catch((e) => {
                if (e?.name !== 'AbortError') setError(e)
            })
            .finally(() => {
                if (!ctl.signal.aborted) setLoading(false)
            })
        return () => ctl.abort()
    }, [key]) // eslint-disable-line react-hooks/exhaustive-deps

    return {
        state,
        data,
        error,
        loading,
        search,
        setSearch,
        setFilter: (filter: string) => update({ filter }),
        setSort: (sort: string) => update({ sort }),
        setPage: (page: number) => update({ page }),
        pageCount: pageCount(data?.total ?? 0, perPage),
        reload: () => setReloads((n) => n + 1),
        perPage,
    }
}
