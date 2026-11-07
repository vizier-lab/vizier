import { beforeEach, expect, test, vi } from 'vitest'

const KEY = 'vizier.sidebar.collapsed'

/** A fresh copy of the store, as a reload would make. */
async function reload() {
    vi.resetModules()
    return (await import('./nav')).useNav
}

beforeEach(() => {
    const items = new Map<string, string>()
    vi.stubGlobal('localStorage', {
        getItem: (k: string) => items.get(k) ?? null,
        setItem: (k: string, v: string) => void items.set(k, v),
    })
})

test('the sidebar starts open', async () => {
    expect((await reload()).getState().sidebarCollapsed).toBe(false)
})

test('a collapsed sidebar stays collapsed after a reload', async () => {
    ;(await reload()).getState().toggleSidebar()
    expect((await reload()).getState().sidebarCollapsed).toBe(true)
})

test('expanding it again is remembered too', async () => {
    const nav = await reload()
    nav.getState().toggleSidebar()
    nav.getState().toggleSidebar()
    expect(localStorage.getItem(KEY)).toBe('false')
    expect((await reload()).getState().sidebarCollapsed).toBe(false)
})

test('blocked storage leaves it working, just not remembered', async () => {
    vi.stubGlobal('localStorage', {
        getItem: () => {
            throw new Error('blocked')
        },
        setItem: () => {
            throw new Error('blocked')
        },
    })
    const nav = await reload()
    expect(nav.getState().sidebarCollapsed).toBe(false)
    nav.getState().toggleSidebar()
    expect(nav.getState().sidebarCollapsed).toBe(true)
})
