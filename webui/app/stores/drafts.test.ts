import { beforeEach, describe, expect, it } from 'vitest'
import { clearDraft, pendingDraft, saveDraft } from './drafts'

const store = new Map<string, string>()
beforeEach(() => {
    store.clear()
    ;(globalThis as { localStorage?: unknown }).localStorage = {
        getItem: (k: string) => store.get(k) ?? null,
        setItem: (k: string, v: string) => void store.set(k, v),
        removeItem: (k: string) => void store.delete(k),
    }
})

describe('drafts', () => {
    it('offers back what differs from the saved text', () => {
        saveDraft('core:plug', 'edited', 'saved')
        expect(pendingDraft('core:plug', 'saved')?.text).toBe('edited')
        expect(pendingDraft('core:plug', 'saved')?.stale).toBe(false)
    })

    it('says a draft is stale once the saved text moved on', () => {
        saveDraft('core:plug', 'edited', 'v1')
        expect(pendingDraft('core:plug', 'v2')?.stale).toBe(true)
    })

    it('forgets a draft that was undone, or cleared', () => {
        saveDraft('a', 'edited', 'saved')
        saveDraft('a', 'saved', 'saved')
        expect(pendingDraft('a', 'saved')).toBeNull()
        saveDraft('b', 'edited', 'saved')
        clearDraft('b')
        expect(pendingDraft('b', 'saved')).toBeNull()
    })

    it('copes with storage that throws', () => {
        ;(globalThis as { localStorage?: unknown }).localStorage = {
            getItem: () => { throw new Error('blocked') },
            setItem: () => { throw new Error('blocked') },
            removeItem: () => { throw new Error('blocked') },
        }
        expect(() => saveDraft('x', 'a', 'b')).not.toThrow()
        expect(pendingDraft('x', 'b')).toBeNull()
    })
})
