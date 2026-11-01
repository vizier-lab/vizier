import { expect, it } from 'vitest'
import { changeHref, changeText, cycleSummary, nightHint, nightLabel } from './dreams'

const night = (over = {}) => ({ date: '2026-10-20', cycle_id: 'c', memories: 3, core: true, core_changes: 1, state: 'complete' as const, ...over })

it('says what a night holds, counting identity changes on their own', () => {
    expect(nightHint(night())).toBe('Oct 20 · 3 memories · 1 identity')
    expect(nightHint(night({ core_changes: 2 }))).toBe('Oct 20 · 3 memories · 2 identity')
    expect(nightHint(night({ memories: 1, core: false, core_changes: 0 }))).toBe('Oct 20 · 1 memory')
    expect(nightHint(night({ cycle_id: null }))).toBe('Oct 20 · no dream')
    expect(nightHint(night({ state: 'running' }))).toContain('running')
})

it('says a night changed identity even when the count is unknown', () => {
    // A night recorded before the count existed still reports that identity moved.
    expect(nightHint(night({ core_changes: 0 }))).toBe('Oct 20 · 3 memories · identity')
})

it('labels the first, every fifth and the last night', () => {
    expect(nightLabel(night(), 0, 30)).toBe('Oct 20')
    expect(nightLabel(night(), 3, 30)).toBeUndefined()
    expect(nightLabel(night(), 29, 30)).toBe('today')
})

it('links every change to its document in Memory', () => {
    expect(changeHref('plug', { kind: 'wrote', bundle: 'infra', path: 'a b.md', seq: 1, at: '' })).toBe('/plug/memory/infra/a%20b')
    // An identity change opens the document it touched, never a page of its own (FR-029).
    expect(changeHref('plug', { kind: 'core', bundle: 'core', path: 'soul', seq: 4, at: '' })).toBe('/plug/memory/core/soul')
    // A change that carries no address still lands on the identity document.
    expect(changeHref('plug', { kind: 'core', bundle: null, path: null, seq: 4, at: '' })).toBe('/plug/memory/core/soul')
    // Any document in the reserved bundle is an identity change (FR-031), so it links there too.
    expect(changeHref('plug', { kind: 'core', bundle: 'core', path: 'values', seq: 5, at: '' })).toBe('/plug/memory/core/values')
})

it('names the address of an identity change', () => {
    expect(changeText({ kind: 'core', bundle: 'core', path: 'soul', seq: 4, at: '' })).toBe('core/soul · rev 4')
    expect(changeText({ kind: 'wrote', bundle: 'infra', path: 'net', seq: 1, at: '' })).toBe('infra/net')
})

it('summarises a cycle, with identity changes excluded from the memories', () => {
    // The backend already excludes them from written/revised/removed (FR-032); the summary must
    // not add them back in.
    expect(cycleSummary({ written: 2, revised: 1, removed: 0, core: true, core_changes: 1 } as never)).toBe('2 written · 1 revised · 1 identity')
    expect(cycleSummary({ written: 2, revised: 1, removed: 0, core: true, core_changes: 2 } as never)).toBe('2 written · 1 revised · 2 identity')
    expect(cycleSummary({ written: 0, revised: 0, removed: 0, core: false, core_changes: 0 } as never)).toBe('no changes')
})
