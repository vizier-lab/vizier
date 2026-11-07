// lint-tokens-exempt: pins colour values.
import { describe, expect, it } from 'vitest'
import { colorProblems } from './contrast'
import { addVariant, dependsOn, derive, duplicateVariant, makeVariant, overrideRole, removeVariant, resetRole, setKey } from './derive'
import { ROLES, type Keys, type Scheme } from './roles'
import { VIZIER_DARK, VIZIER_LIGHT } from './vizier'

const KEYS: Keys = { content: '#1e1e2e', frame: '#181825', text: '#cdd6f4', accent: '#cba6f7', live: '#a6e3a1', retrying: '#f9e2af', failed: '#f38ba8' }
const LIGHT_KEYS: Keys = { content: '#ffffff', frame: '#f1f1f3', text: '#111114', accent: '#3651c4', live: '#046c4e', retrying: '#8a5a10', failed: '#b4321c' }

describe('derive', () => {
    it('gives every role a colour', () => {
        const roles = derive(KEYS)
        for (const role of ROLES) expect(roles[role], role).toMatch(/^#[0-9a-f]{6}$/)
    })

    it('uses each key for its own role', () => {
        const roles = derive(KEYS)
        expect(roles['bg/panel']).toBe(KEYS.content)
        expect(roles['bg/shell']).toBe(KEYS.frame)
        expect(roles['text/default']).toBe(KEYS.text)
        expect(roles['fill/ink']).toBe(KEYS.accent)
        expect(roles['state/live']).toBe(KEYS.live)
        expect(roles['state/retrying']).toBe(KEYS.retrying)
        expect(roles['state/failed']).toBe(KEYS.failed)
    })

    it.each([
        ['dark', KEYS],
        ['light', LIGHT_KEYS],
    ] as const)('derives a legible %s variant from sensible keys', (_name, keys) => {
        expect(colorProblems(derive(keys)).map((p) => p.message)).toEqual([])
    })

    it('keeps a grey accent\'s code in shades of grey', () => {
        const roles = derive({ ...KEYS, accent: '#f2f2f5' })
        for (const hex of [roles['code/keyword'], roles['code/string'], roles['code/number']]) {
            const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16))
            expect(Math.max(r, g, b) - Math.min(r, g, b)).toBeLessThan(24)
        }
    })
})

describe('editing keys', () => {
    it('recomputes only the dependents of the key', () => {
        const before = makeVariant(KEYS)
        const after = setKey(before, 'accent', '#89b4fa')
        expect(after.roles['fill/ink']).toBe('#89b4fa')
        expect(after.roles['code/keyword']).not.toBe(before.roles['code/keyword'])
        expect(after.roles['bg/panel']).toBe(before.roles['bg/panel'])
        expect(after.roles['state/live']).toBe(before.roles['state/live'])
    })

    // SC-010: a status colour never moves a code colour.
    it.each(['live', 'retrying', 'failed'] as const)('leaves code alone when %s changes', (key) => {
        const before = makeVariant(KEYS)
        const after = setKey(before, key, '#336699')
        for (const role of ROLES.filter((r) => r.startsWith('code/'))) expect(after.roles[role], role).toBe(before.roles[role])
    })

    it('leaves status colours alone when a code colour is overridden', () => {
        const before = makeVariant(KEYS)
        const after = overrideRole(before, 'code/keyword', '#336699')
        for (const role of ROLES.filter((r) => r !== 'code/keyword')) expect(after.roles[role], role).toBe(before.roles[role])
    })

    it('keeps a preset\'s own colours when an unrelated key changes', () => {
        const before = makeVariant(KEYS, { 'code/number': '#fab387', 'bg/surface': '#313244' })
        const after = setKey(before, 'live', '#40a02b')
        expect(after.roles['code/number']).toBe('#fab387')
        expect(after.roles['bg/surface']).toBe('#313244')
    })

    it('lets every dependency name real keys', () => {
        for (const role of ROLES) expect(dependsOn(role).length, role).toBeGreaterThan(0)
    })
})

describe('overrides', () => {
    it('survive a key change, marked, until reset', () => {
        const edited = overrideRole(makeVariant(KEYS), 'code/keyword', '#336699')
        expect(edited.overridden).toEqual(['code/keyword'])
        const moved = setKey(edited, 'accent', '#89b4fa')
        expect(moved.roles['code/keyword']).toBe('#336699')
        expect(moved.overridden).toEqual(['code/keyword'])
        const reset = resetRole(moved, 'code/keyword')
        expect(reset.overridden).toEqual([])
        expect(reset.roles['code/keyword']).toBe(derive(moved.keys)['code/keyword'])
    })

    it('moves the key when the role is a key\'s own', () => {
        const edited = overrideRole(makeVariant(KEYS), 'state/live', '#336699')
        expect(edited.keys.live).toBe('#336699')
        expect(edited.roles['state/live']).toBe('#336699')
    })
})

describe('duplicate', () => {
    it('is identical, with nothing marked, until a key changes', () => {
        const source = overrideRole(makeVariant(KEYS, { 'code/number': '#fab387' }), 'code/string', '#336699')
        const copy = duplicateVariant(source)
        expect(copy.keys).toEqual(source.keys)
        expect(copy.roles).toEqual(source.roles)
        expect(copy.overridden).toEqual([])
        expect(copy.roles).not.toBe(source.roles)
        const moved = setKey(copy, 'accent', '#89b4fa')
        expect(moved.roles['code/string']).not.toBe('#336699')
        expect(moved.roles['bg/panel']).toBe(source.roles['bg/panel'])
    })

    it('copies Vizier exactly', () => {
        expect(duplicateVariant(VIZIER_LIGHT).roles).toEqual(VIZIER_LIGHT.roles)
        expect(duplicateVariant(VIZIER_DARK).roles).toEqual(VIZIER_DARK.roles)
    })
})

describe('variants', () => {
    const darkOnly: Scheme = { id: 'custom-1', name: 'Mine', kind: 'custom', dark: makeVariant(KEYS) }

    it('adds the missing one from the base keys, keeping accent and live', () => {
        const both = addVariant(darkOnly, 'light', VIZIER_LIGHT.keys)
        expect(both.light!.keys.content).toBe(VIZIER_LIGHT.keys.content)
        expect(both.light!.keys.accent).toBe(KEYS.accent)
        expect(both.light!.keys.live).toBe(KEYS.live)
        expect(both.dark).toBe(darkOnly.dark)
    })

    it('never replaces one that exists', () => {
        expect(addVariant(darkOnly, 'dark', VIZIER_DARK.keys)).toBe(darkOnly)
    })

    it('removes one, but never the last', () => {
        const both = addVariant(darkOnly, 'light', VIZIER_LIGHT.keys)
        const one = removeVariant(both, 'dark')
        expect(one.dark).toBeUndefined()
        expect(one.light).toBeDefined()
        expect(removeVariant(one, 'light')).toBe(one)
    })
})
