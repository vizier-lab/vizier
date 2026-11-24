// lint-tokens-exempt: pins colour values.
import { describe, expect, it } from 'vitest'
import { fileNameFor, parse, serialize } from './file'
import { PRESETS, presetById } from './presets'
import { MAX_CUSTOM_SCHEMES, ROLES, type Scheme } from './roles'
import { VIZIER_DARK } from './vizier'

const none: Scheme[] = []
const refusal = (text: string, existing = none) => {
    const result = parse(text, existing)
    return result.ok ? null : result.error
}
const theme = (body: object) => JSON.stringify({ format: 'vizier-theme', version: 1, name: 'Mine', ...body })

describe('export', () => {
    it('lists every role of every variant, and nothing private', () => {
        const file = JSON.parse(serialize(presetById('nord')!))
        expect(Object.keys(file)).toEqual(['format', 'version', 'name', 'dark'])
        expect(Object.keys(file.dark)).toEqual(['keys', 'roles'])
        expect(Object.keys(file.dark.roles)).toEqual([...ROLES])
        expect(file.format).toBe('vizier-theme')
        expect(file.version).toBe(1)
    })

    it('is readable', () => {
        expect(serialize(presetById('vizier')!)).toContain('\n    "roles": {\n      "bg/ground": "#ffffff",')
    })

    it('names the file from the theme', () => {
        expect(fileNameFor('Rosé Pine')).toBe('rose-pine.vizier-theme.json')
        expect(fileNameFor('!!!')).toBe('theme.vizier-theme.json')
    })
})

describe('export then import', () => {
    // SC-006: every colour exactly.
    it.each(PRESETS.map((p) => [p.name, p] as const))('reproduces %s', (_name, preset) => {
        const result = parse(serialize(preset), none)
        expect(result.ok).toBe(true)
        if (!result.ok) return
        expect(result.scheme.light?.roles).toEqual(preset.light?.roles)
        expect(result.scheme.dark?.roles).toEqual(preset.dark?.roles)
        expect(result.scheme.light?.keys).toEqual(preset.light?.keys)
        expect(result.scheme.dark?.keys).toEqual(preset.dark?.keys)
    })

    it('always makes a new custom scheme with no overrides', () => {
        const result = parse(serialize(presetById('dracula')!), none)
        if (!result.ok) throw new Error(result.error)
        expect(result.scheme.kind).toBe('custom')
        expect(result.scheme.id).toMatch(/^custom-/)
        expect(result.scheme.dark!.overridden).toEqual([])
        expect(result.scheme.paints).toBeUndefined()
    })
})

describe('refusals, in order', () => {
    it.each([
        ['not JSON', '\u0089PNG\r\n', "That file isn't valid JSON."],
        ['another kind of file', '{"format":"other"}', "That file isn't a Vizier theme."],
        ['an array', '[]', "That file isn't a Vizier theme."],
        ['null', 'null', "That file isn't a Vizier theme."],
        ['a newer version', theme({ version: 2 }), 'That theme needs a newer version of Vizier.'],
        ['no name', JSON.stringify({ format: 'vizier-theme', version: 1, dark: {} }), 'Enter a theme name of 1 to 40 characters.'],
        ['an empty name', theme({ name: '   ', dark: {} }), 'Enter a theme name of 1 to 40 characters.'],
        ['a long name', theme({ name: 'x'.repeat(41), dark: {} }), 'Enter a theme name of 1 to 40 characters.'],
        ['a name that is not text', theme({ name: { a: 1 }, dark: {} }), 'Enter a theme name of 1 to 40 characters.'],
        ['no variants', theme({}), 'That theme has no light or dark colors.'],
        ['a variant that is not an object', theme({ dark: 5 }), 'That theme has no light or dark colors.'],
        ['a role that is not a colour', theme({ dark: { roles: { 'bg/panel': 'red' } } }), "dark.roles.bg/panel isn't a color. Use a value like #1e1e2e."],
        ['CSS in a value', theme({ dark: { roles: { 'bg/panel': 'red; background:url(x)' } } }), "dark.roles.bg/panel isn't a color. Use a value like #1e1e2e."],
        ['a script url', theme({ light: { keys: { text: 'url(javascript:alert(1))' } } }), "light.keys.text isn't a color. Use a value like #1e1e2e."],
        ['short hex', theme({ dark: { keys: { text: '#fff' } } }), "dark.keys.text isn't a color. Use a value like #1e1e2e."],
        ['alpha hex', theme({ dark: { keys: { text: '#ffffff80' } } }), "dark.keys.text isn't a color. Use a value like #1e1e2e."],
        ['keys that are not an object', theme({ dark: { keys: 'x' } }), "dark.keys isn't a list of colors."],
    ])('%s', (_name, text, message) => {
        expect(refusal(text)).toBe(message)
    })

    it('a file over the size limit', () => {
        expect(refusal(theme({ dark: {}, padding: 'x'.repeat(256 * 1024) }))).toBe('That file is too large to be a theme.')
    })

    it('at the limit of custom schemes', () => {
        const many = Array.from({ length: MAX_CUSTOM_SCHEMES }, (_, i) => ({ id: `custom-${i}`, name: `T${i}`, kind: 'custom' as const }))
        expect(refusal(theme({ dark: {} }), many)).toBe("You've reached 50 themes. Delete one to import this.")
    })
})

describe('hostile files', () => {
    it('ignores what it does not know', () => {
        const text = '{"format":"vizier-theme","version":1,"name":"Mine","script":"alert(1)","__proto__":{"polluted":true},"dark":{"__proto__":{"polluted":true},"keys":{"__proto__":"#000000","nope":"red","text":"#eeeeee"},"roles":{"constructor":"red","not/a-role":"url(x)","bg/panel":"#101010"}}}'
        const result = parse(text, none)
        if (!result.ok) throw new Error(result.error)
        expect(({} as Record<string, unknown>).polluted).toBeUndefined()
        expect(Object.getPrototypeOf(result.scheme.dark!.roles)).toBe(Object.prototype)
        expect(Object.keys(result.scheme.dark!.roles).sort()).toEqual([...ROLES].sort())
        for (const value of Object.values(result.scheme.dark!.roles)) expect(value).toMatch(/^#[0-9a-f]{6}$/)
        expect(result.scheme.dark!.keys.text).toBe('#eeeeee')
        expect(result.scheme.dark!.keys.content).toBe('#101010')
        expect(JSON.stringify(result.scheme)).not.toContain('script')
    })
})

describe('filling gaps', () => {
    it('derives the rest from a few keys', () => {
        const result = parse(theme({ dark: { keys: { content: '#101018', text: '#e0e0f0', accent: '#8899ff' } } }), none)
        if (!result.ok) throw new Error(result.error)
        const dark = result.scheme.dark!
        expect(dark.roles['bg/panel']).toBe('#101018')
        expect(dark.roles['fill/ink']).toBe('#8899ff')
        expect(dark.keys.live).toBe(VIZIER_DARK.keys.live)
        expect(dark.overridden).toEqual([])
        for (const role of ROLES) expect(dark.roles[role], role).toMatch(/^#[0-9a-f]{6}$/)
    })

    it('marks the roles a partial file gave, and takes a key from its role', () => {
        const result = parse(theme({ dark: { roles: { 'text/default': '#eeeeee', 'code/keyword': '#ff00ff' } } }), none)
        if (!result.ok) throw new Error(result.error)
        expect(result.scheme.dark!.keys.text).toBe('#eeeeee')
        expect(result.scheme.dark!.roles['code/keyword']).toBe('#ff00ff')
        expect(result.scheme.dark!.overridden.sort()).toEqual(['code/keyword', 'text/default'])
    })

    it('keeps a hand edit in a whole file, unmarked', () => {
        const file = JSON.parse(serialize(presetById('nord')!))
        file.name = 'My Nord'
        file.dark.roles['code/keyword'] = '#123456'
        const result = parse(JSON.stringify(file), PRESETS)
        if (!result.ok) throw new Error(result.error)
        expect(result.scheme.name).toBe('My Nord')
        expect(result.scheme.dark!.roles['code/keyword']).toBe('#123456')
        expect(result.scheme.dark!.overridden).toEqual([])
    })
})

describe('name clashes', () => {
    const named = (...names: string[]) => names.map((name, i) => ({ id: `x${i}`, name, kind: 'preset' as const }))

    it('adds a suffix and overwrites nothing', () => {
        const result = parse(theme({ name: 'Vizier', dark: {} }), named('Vizier'))
        expect(result.ok && result.scheme.name).toBe('Vizier (2)')
        const third = parse(theme({ name: 'vizier', dark: {} }), named('Vizier', 'vizier (2)'))
        expect(third.ok && third.scheme.name).toBe('vizier (3)')
    })

    it('stays within the limit', () => {
        const long = 'x'.repeat(40)
        const result = parse(theme({ name: long, dark: {} }), named(long))
        expect(result.ok && result.scheme.name).toBe(`${'x'.repeat(36)} (2)`)
    })
})
