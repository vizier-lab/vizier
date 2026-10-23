import { describe, expect, it } from 'vitest'
import { pageCount, parseListState, rangeLabel, toListQuery, writeListState } from './paging'

describe('paging', () => {
    it('round-trips list state through the URL, omitting defaults', () => {
        const defaults = { sort: 'next_run' }
        const state = parseListState(new URLSearchParams('q=nightly&filter=active&page=3'), defaults)
        expect(state).toMatchObject({ q: 'nightly', filter: 'active', sort: 'next_run', page: 3 })
        const back = writeListState(new URLSearchParams(), state, defaults)
        expect(back.toString()).toBe('q=nightly&filter=active&page=3')
    })

    it('ignores a bad page', () => {
        expect(parseListState(new URLSearchParams('page=0')).page).toBe(1)
        expect(parseListState(new URLSearchParams('page=x')).page).toBe(1)
    })

    it('builds the server query from a page', () => {
        const q = toListQuery({ q: ' a ', filter: 'all', sort: '', page: 3 })
        expect(q).toEqual({ q: 'a', filter: undefined, sort: undefined, order: undefined, offset: 50, limit: 25 })
    })

    it('does the page math', () => {
        expect(pageCount(0)).toBe(1)
        expect(pageCount(312)).toBe(13)
        expect(rangeLabel(1, 312)).toBe('1–25 of 312')
        expect(rangeLabel(13, 312)).toBe('301–312 of 312')
        expect(rangeLabel(1, 1500)).toBe('1–25 of 1,500')
        expect(rangeLabel(1, 0)).toBe('0 of 0')
    })
})
