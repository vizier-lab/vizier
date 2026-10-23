import { expect, it } from 'vitest'
import { changeHref, cycleSummary, nightHint, nightLabel } from './dreams'

const night = (over = {}) => ({ date: '2026-10-20', cycle_id: 'c', memories: 3, core: true, state: 'complete' as const, ...over })

it('says what a night holds', () => {
    expect(nightHint(night())).toBe('Oct 20 · 3 memories · CORE')
    expect(nightHint(night({ memories: 1, core: false }))).toBe('Oct 20 · 1 memory')
    expect(nightHint(night({ cycle_id: null }))).toBe('Oct 20 · no dream')
    expect(nightHint(night({ state: 'running' }))).toContain('running')
})

it('labels the first, every fifth and the last night', () => {
    expect(nightLabel(night(), 0, 30)).toBe('Oct 20')
    expect(nightLabel(night(), 3, 30)).toBeUndefined()
    expect(nightLabel(night(), 29, 30)).toBe('today')
})

it('links a change to its document or to CORE', () => {
    expect(changeHref('plug', { kind: 'wrote', bundle: 'infra', path: 'a b.md', seq: 1, at: '' })).toBe('/plug/memory/infra?doc=a%20b.md')
    expect(changeHref('plug', { kind: 'core', bundle: null, path: null, seq: 4, at: '' })).toBe('/plug/core')
})

it('summarises a cycle', () => {
    expect(cycleSummary({ written: 2, revised: 1, removed: 0, core: true } as never)).toBe('2 written · 1 revised · CORE')
    expect(cycleSummary({ written: 0, revised: 0, removed: 0, core: false } as never)).toBe('no changes')
})
