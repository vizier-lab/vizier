import { beforeEach, describe, expect, it, vi } from 'vitest'

// The tests run in node: a memory-backed storage and a window that only carries events.
class MemoryStorage {
    private items = new Map<string, string>()
    getItem = (key: string) => this.items.get(key) ?? null
    setItem = (key: string, value: string) => void this.items.set(key, String(value))
    removeItem = (key: string) => void this.items.delete(key)
    clear = () => this.items.clear()
}
const localStorage = new MemoryStorage()
vi.stubGlobal('localStorage', localStorage)
vi.stubGlobal('window', new EventTarget())

// The marker is read once, when the module loads: each test loads it afresh over prepared storage.
async function load(stored?: Record<string, unknown>) {
    vi.resetModules()
    localStorage.clear()
    if (stored) localStorage.setItem('vz.mounting', JSON.stringify(stored))
    return import('./mounting')
}

describe('the crash-loop marker', () => {
    beforeEach(() => localStorage.clear())

    it('pauses a version a previous tab left marked, and tells only its conversation', async () => {
        const m = await load({ 'pg_1:3': { page: 'pg_1', version: 3, topic: 'a/t', at: 1 } })
        expect(m.isPaused('pg_1', 3)).toBe(true)
        expect(m.isPaused('pg_1', 2)).toBe(false)
        expect(m.pausedNotes('a/t')).toEqual([expect.stringContaining('Page pg_1 v3 froze')])
        expect(m.pausedNotes('a/other')).toEqual([])
    })

    it('runs a paused version anyway and forgets it', async () => {
        const m = await load({ 'pg_1:3': { page: 'pg_1', version: 3, topic: 'a/t', at: 1 } })
        const changed = vi.fn()
        m.onPausedChange(changed)
        m.runAnyway('pg_1', 3)
        expect(m.isPaused('pg_1', 3)).toBe(false)
        expect(changed).toHaveBeenCalled()
        expect(localStorage.getItem('vz.mounting')).toBeNull()
    })

    it('marks a mount for the next load and unmarks it when it runs', async () => {
        const m = await load()
        m.markMounting('pg_2', 1, 'a/t')
        expect(JSON.parse(localStorage.getItem('vz.mounting') ?? '{}')).toHaveProperty(['pg_2:1'])
        expect(m.isPaused('pg_2', 1)).toBe(false)
        m.clearMounting('pg_2', 1)
        expect(localStorage.getItem('vz.mounting')).toBeNull()
    })

    it('unmarks what this tab mounted when the page is left normally', async () => {
        const m = await load({ 'pg_1:3': { page: 'pg_1', version: 3, topic: 'a/t', at: 1 } })
        m.markMounting('pg_2', 1, 'a/t')
        window.dispatchEvent(new Event('pagehide'))
        expect(Object.keys(JSON.parse(localStorage.getItem('vz.mounting') ?? '{}'))).toEqual(['pg_1:3'])
    })
})
