// lint-tokens-exempt: pins palette values.
import { describe, expect, it } from 'vitest'
import { colorProblems } from './contrast'
import { appearancesOf, presetById, PRESETS, schemeDetail } from './presets'
import { KEY_COLORS, KEY_ROLE, ROLES } from './roles'
import { VIZIER_DARK, VIZIER_LIGHT } from './vizier'

const variants = PRESETS.flatMap((scheme) => appearancesOf(scheme).map((appearance) => [`${scheme.name} ${appearance}`, scheme[appearance]!] as const))

describe('presets', () => {
    it('are the agreed eleven, with the agreed variants (FR-002)', () => {
        const both = ['Vizier', 'Vizier Swapped', 'Oxocarbon', 'Catppuccin', 'Gruvbox', 'Rosé Pine', 'Tokyo Night', 'Everforest', 'Solarized']
        expect(PRESETS.map((p) => p.name).sort()).toEqual([...both, 'Nord', 'Dracula'].sort())
        for (const name of both) {
            const scheme = PRESETS.find((p) => p.name === name)!
            expect(appearancesOf(scheme)).toEqual(['light', 'dark'])
        }
        for (const id of ['nord', 'dracula']) {
            expect(appearancesOf(presetById(id)!)).toEqual(['dark'])
            expect(schemeDetail(presetById(id)!)).toBe('Dark only')
        }
    })

    it('have unique ids and names', () => {
        expect(new Set(PRESETS.map((p) => p.id)).size).toBe(PRESETS.length)
        expect(new Set(PRESETS.map((p) => p.name.toLowerCase())).size).toBe(PRESETS.length)
    })

    it.each(variants)('%s is complete and only hex', (_name, variant) => {
        for (const role of ROLES) expect(variant.roles[role], role).toMatch(/^#[0-9a-f]{6}$/)
        for (const key of KEY_COLORS) expect(variant.keys[key], key).toBe(variant.roles[KEY_ROLE[key]])
    })

    // SC-005: body text and code 4.5:1, comments and marks 3:1, accent and live clearly apart.
    it.each(variants)('%s meets the contrast rule', (_name, variant) => {
        expect(colorProblems(variant.roles).map((p) => p.message)).toEqual([])
    })

    it('keep code colours apart from status colours (FR-008a)', () => {
        for (const [name, variant] of variants) {
            expect(variant.roles['code/keyword'], name).toBeDefined()
        }
        const catppuccin = presetById('catppuccin')!.dark!
        expect(catppuccin.roles['code/number']).toBe('#fab387')
        expect(catppuccin.roles['state/retrying']).toBe('#f9e2af')
    })

    it('stay on published colours where those already pass', () => {
        const dracula = presetById('dracula')!.dark!
        expect(dracula.keys).toEqual({ content: '#282a36', frame: '#21222c', text: '#f8f8f2', accent: '#bd93f9', live: '#50fa7b', retrying: '#f1fa8c', failed: '#ff5555' })
        const solarized = presetById('solarized')!.dark!
        expect(solarized.keys.content).toBe('#002b36')
        expect(solarized.keys.live).toBe('#859900')
    })
})

describe('Vizier', () => {
    const vizier = presetById('vizier')!

    it('is the stylesheet, and paints nothing over it', () => {
        expect(vizier.light).toBe(VIZIER_LIGHT)
        expect(vizier.dark).toBe(VIZIER_DARK)
        expect(vizier.paints).toEqual({ light: [], dark: [] })
    })

    // FR-008c: neutrals with emerald on strings only, and a range of shades.
    it.each([
        ['light', VIZIER_LIGHT],
        ['dark', VIZIER_DARK],
    ] as const)('has monochrome code in %s', (_name, variant) => {
        const { 'code/string': string, ...rest } = Object.fromEntries(ROLES.filter((r) => r.startsWith('code/')).map((r) => [r, variant.roles[r]]))
        expect(string).toBe(variant.roles['state/live-text'])
        const shades = new Set(Object.values(rest))
        expect(shades.size).toBe(5)
        for (const hex of shades) {
            const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex!.slice(i, i + 2), 16))
            expect(Math.max(r, g, b) - Math.min(r, g, b), hex).toBeLessThan(16)
        }
    })
})

describe('Vizier Swapped (FR-002a)', () => {
    const swapped = presetById('vizier-swapped')!

    it('exchanges frame and inset exactly', () => {
        for (const [variant, base] of [
            [swapped.light!, VIZIER_LIGHT],
            [swapped.dark!, VIZIER_DARK],
        ] as const) {
            expect(variant.roles['bg/shell']).toBe(base.roles['bg/panel'])
            expect(variant.roles['bg/panel']).toBe(base.roles['bg/shell'])
            expect(variant.keys.frame).toBe(base.keys.content)
            expect(variant.keys.content).toBe(base.keys.frame)
        }
    })

    it('paints only what it changes', () => {
        expect(swapped.paints!.dark!.sort()).toEqual(['bg/panel', 'bg/shell'])
        expect(swapped.paints!.light!.sort()).toEqual(['bg/panel', 'bg/shell', 'bg/surface', 'text/faint'])
    })

    it('keeps cards apart from the inset, and Vizier\'s code colours', () => {
        expect(swapped.light!.roles['bg/surface']).toBe('#ffffff')
        for (const role of ROLES.filter((r) => r.startsWith('code/'))) {
            expect(swapped.light!.roles[role]).toBe(VIZIER_LIGHT.roles[role])
            expect(swapped.dark!.roles[role]).toBe(VIZIER_DARK.roles[role])
        }
    })
})
