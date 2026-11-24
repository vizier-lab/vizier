// lint-tokens-exempt: pins colour values.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { makeVariant } from '../lib/themes/derive'
import { nameProblem, parseCustoms, serializeCustoms } from '../lib/themes/customs'
import { MAX_CUSTOM_SCHEMES, ROLES, type Keys, type Scheme } from '../lib/themes/roles'
import { hasBothVariants } from './appearance'

const KEYS: Keys = { content: '#1e1e2e', frame: '#181825', text: '#cdd6f4', accent: '#cba6f7', live: '#a6e3a1', retrying: '#f9e2af', failed: '#f38ba8' }
const mine = (id = 'custom-1', name = 'Mine'): Scheme => ({ id, name, kind: 'custom', dark: makeVariant(KEYS) })

const toasts: string[] = []
vi.mock('./toasts', () => ({ toast: { error: (title: string) => toasts.push(title), success: () => {}, info: () => {} } }))
vi.mock('../api/transport', () => ({ transport: { authMode: 'http' }, transportReady: Promise.resolve() }))

function fakeBrowser(stored: Record<string, string> = {}) {
    const store = new Map(Object.entries(stored))
    const state = { failing: false }
    const style = new Map<string, string>()
    const attrs = new Map<string, string>()
    const meta = new Map<string, string>()
    const listeners = new Map<string, (e: unknown) => void>()
    vi.stubGlobal('localStorage', {
        getItem: (k: string) => store.get(k) ?? null,
        setItem: (k: string, v: string) => {
            if (state.failing) throw new Error('full')
            store.set(k, v)
        },
        removeItem: (k: string) => {
            if (state.failing) throw new Error('full')
            store.delete(k)
        },
    })
    vi.stubGlobal('window', {
        matchMedia: () => ({ matches: true, addEventListener: () => {}, removeEventListener: () => {} }),
        addEventListener: (k: string, f: (e: unknown) => void) => listeners.set(k, f),
        removeEventListener: () => {},
    })
    vi.stubGlobal('document', {
        documentElement: {
            style: { setProperty: (n: string, v: string) => void style.set(n, v), removeProperty: (n: string) => (style.delete(n), '') },
            setAttribute: (n: string, v: string) => void attrs.set(n, v),
            removeAttribute: (n: string) => void attrs.delete(n),
        },
        querySelector: (s: string) =>
            s === 'meta[name="theme-color"]'
                ? { setAttribute: (n: string, v: string) => void meta.set(n, v) }
                : null,
    })
    return { store, state, style, attrs, meta, listeners }
}

async function freshStore() {
    vi.resetModules()
    const { useAppearance } = await import('./appearance')
    return useAppearance
}

beforeEach(() => {
    toasts.length = 0
    vi.unstubAllGlobals()
})

describe('reading custom schemes', () => {
    it.each([null, '', '{oops', 'null', '[]', '{"schemes":5}'])('reads %s as none', (raw) => {
        expect(parseCustoms(raw)).toEqual([])
    })

    it('drops only the damaged entries', () => {
        const raw = JSON.stringify({
            version: 1,
            schemes: [mine('custom-a', 'A'), { id: 'custom-b' }, { id: 'nope', name: 'C', dark: mine().dark }, { id: 'custom-d', name: 'D' }, 5, null, mine('custom-e', 'E')],
        })
        expect(parseCustoms(raw).map((s) => s.name)).toEqual(['A', 'E'])
    })

    it('round trips', () => {
        const scheme = mine()
        const [back] = parseCustoms(serializeCustoms([scheme]))
        expect(back).toEqual(scheme)
    })

    it('fills a role an earlier version did not have (FR-026)', () => {
        const scheme = mine()
        const { 'code/keyword': _gone, ...roles } = scheme.dark!.roles
        const [back] = parseCustoms(JSON.stringify({ version: 1, schemes: [{ ...scheme, dark: { ...scheme.dark, roles } }] }))
        for (const role of ROLES) expect(back.dark!.roles[role], role).toMatch(/^#[0-9a-f]{6}$/)
    })

    it('drops a stored value that is not a colour', () => {
        const scheme = mine()
        const [back] = parseCustoms(JSON.stringify({ version: 1, schemes: [{ ...scheme, dark: { ...scheme.dark, roles: { ...scheme.dark!.roles, 'bg/panel': 'red;x:y' } } }] }))
        expect(back.dark!.roles['bg/panel']).toBe(KEYS.content)
    })
})

describe('names', () => {
    const schemes = [mine('custom-1', 'Mine'), { id: 'nord', name: 'Nord', kind: 'preset' as const }]

    it('are unique among all schemes, ignoring case', () => {
        expect(nameProblem('NORD', schemes)).toBe('That name is already taken. Try another.')
        expect(nameProblem(' mine ', schemes)).toBe('That name is already taken. Try another.')
        expect(nameProblem('Fresh', schemes)).toBeNull()
    })

    it('a scheme may keep its own', () => {
        expect(nameProblem('Mine', schemes, 'custom-1')).toBeNull()
    })

    it('are 1 to 40 characters after trimming', () => {
        expect(nameProblem('   ', schemes)).toBe('Enter a theme name of 1 to 40 characters.')
        expect(nameProblem('x'.repeat(41), schemes)).toBe('Enter a theme name of 1 to 40 characters.')
        expect(nameProblem(`  ${'x'.repeat(40)}  `, schemes)).toBeNull()
    })
})

describe('the store', () => {
    it('falls back to Vizier for a selected id that is gone', async () => {
        const { store, style } = fakeBrowser({ 'vizier.scheme': 'gone', 'vizier.scheme.paint': '{"id":"gone","dark":{"--vz-panel":"#000000"}}' })
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        useAppearance.getState().settle()
        expect(useAppearance.getState().schemeId).toBe('vizier')
        expect(store.has('vizier.scheme')).toBe(false)
        expect(store.has('vizier.scheme.paint')).toBe(false)
        expect(style.size).toBe(0)
    })

    it('survives damaged data on every key', async () => {
        fakeBrowser({ 'vizier.scheme': 'custom-1', 'vizier.schemes': '{oops', 'vizier.scheme.paint': '{oops', 'vizier.theme': 'purple' })
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        useAppearance.getState().settle()
        expect(useAppearance.getState()).toMatchObject({ schemeId: 'vizier', mode: 'system', customs: [] })
    })

    it('keeps a change for the session when a write fails, with one toast', async () => {
        const { state, style } = fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        state.failing = true
        useAppearance.getState().select('nord')
        expect(useAppearance.getState().schemeId).toBe('nord')
        expect(style.get('--vz-panel')).toBe('#3b4252')
        expect(toasts).toEqual(["Your theme couldn't be saved in this browser."])
    })

    it('saves, re-applies and deletes a custom scheme', async () => {
        const { store, style } = fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        expect(useAppearance.getState().saveCustom(mine())).toBeNull()
        expect(parseCustoms(store.get('vizier.schemes')!)).toHaveLength(1)
        // Saved, not selected: the page is unchanged.
        expect(style.size).toBe(0)

        useAppearance.getState().select('custom-1')
        expect(style.get('--vz-panel')).toBe(KEYS.content)

        // Saving the selected one re-applies it and rewrites the cache.
        const scheme = mine()
        const edited = { ...scheme, dark: makeVariant({ ...KEYS, content: '#000011' }) }
        expect(useAppearance.getState().saveCustom(edited)).toBeNull()
        expect(style.get('--vz-panel')).toBe('#000011')
        expect(store.get('vizier.scheme.paint')).toContain('#000011')

        // Deleting the selected one returns to Vizier.
        useAppearance.getState().deleteCustom('custom-1')
        expect(useAppearance.getState().schemeId).toBe('vizier')
        expect(useAppearance.getState().customs).toEqual([])
        expect(style.size).toBe(0)
        expect(store.has('vizier.scheme.paint')).toBe(false)
    })

    it('paints the browser chrome when the scheme changes', async () => {
        const { meta } = fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        useAppearance.getState().setMode('dark')
        expect(meta.get('content')).toBe('#0b0b0c')
        useAppearance.getState().setMode('light')
        expect(meta.get('content')).toBe('#ffffff')
    })

    it('refuses a taken name and the fifty-first scheme', async () => {
        fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        expect(useAppearance.getState().saveCustom(mine('custom-x', 'nord'))).toBe('That name is already taken. Try another.')
        for (let i = 0; i < MAX_CUSTOM_SCHEMES; i++) expect(useAppearance.getState().saveCustom(mine(`custom-${i}`, `T${i}`))).toBeNull()
        expect(useAppearance.getState().saveCustom(mine('custom-more', 'One more'))).toBe("You've reached 50 themes. Delete one to make another.")
        // An edit is not a new one.
        expect(useAppearance.getState().saveCustom(mine('custom-0', 'Renamed'))).toBeNull()
    })

    it('switches light and dark from what is showing, for a scheme with both', async () => {
        const { store, attrs } = fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        expect(hasBothVariants(useAppearance.getState().paint)).toBe(true)
        // The device is dark, so the first press goes to light, then back.
        useAppearance.getState().toggleMode()
        expect(useAppearance.getState().mode).toBe('light')
        expect(attrs.get('data-theme')).toBe('light')
        expect(store.get('vizier.theme')).toBe('light')
        useAppearance.getState().toggleMode()
        expect(useAppearance.getState().mode).toBe('dark')
    })

    it('offers no switch for a scheme with one variant, and keeps the choice for later', async () => {
        fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        useAppearance.getState().select('catppuccin')
        expect(hasBothVariants(useAppearance.getState().paint)).toBe(true)
        useAppearance.getState().select('nord')
        expect(hasBothVariants(useAppearance.getState().paint)).toBe(false)
    })

    it('follows a change made in another tab', async () => {
        const { store, style, listeners } = fakeBrowser()
        const useAppearance = await freshStore()
        await useAppearance.getState().load()
        useAppearance.getState().init()
        store.set('vizier.scheme', 'nord')
        listeners.get('storage')!({ key: 'vizier.scheme' })
        await vi.waitFor(() => expect(style.get('--vz-panel')).toBe('#3b4252'))
        expect(useAppearance.getState().schemeId).toBe('nord')
    })
})
