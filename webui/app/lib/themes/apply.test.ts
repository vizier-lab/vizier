// lint-tokens-exempt: pins colour values.
import { describe, expect, it } from 'vitest'
import { applyTo, appearanceOf, paintOf, PRE_PAINT_SCRIPT, readPaint, resolve, resolvePaint } from './apply'
import { presetById } from './presets'

const vizier = presetById('vizier')!
const swapped = presetById('vizier-swapped')!
const catppuccin = presetById('catppuccin')!
const nord = presetById('nord')!

function fakeElement() {
    const style = new Map<string, string>()
    const attrs = new Map<string, string>()
    return {
        vars: style,
        attrs,
        style: {
            setProperty: (n: string, v: string) => void style.set(n, v),
            removeProperty: (n: string) => (style.delete(n), ''),
        } as unknown as CSSStyleDeclaration,
        setAttribute: (n: string, v: string) => void attrs.set(n, v),
        removeAttribute: (n: string) => void attrs.delete(n),
    }
}

describe('which variant shows (data-model)', () => {
    it('one variant: always it, and data-theme is forced', () => {
        for (const [mode, device] of [
            ['system', false],
            ['light', false],
            ['dark', true],
        ] as const) {
            const r = resolve(nord, mode, device)
            expect(r.appearance).toBe('dark')
            expect(r.dataTheme).toBe('dark')
            expect(r.vars['--vz-panel']).toBe(nord.dark!.roles['bg/panel'])
        }
    })

    it('both, fixed mode: that variant, whatever the device says', () => {
        const r = resolve(catppuccin, 'light', true)
        expect(r.appearance).toBe('light')
        expect(r.dataTheme).toBe('light')
        expect(r.vars['--vz-panel']).toBe('#eff1f5')
    })

    it('both, system: the device, with data-theme unset', () => {
        expect(resolve(catppuccin, 'system', true)).toMatchObject({ appearance: 'dark', dataTheme: null })
        expect(resolve(catppuccin, 'system', false)).toMatchObject({ appearance: 'light', dataTheme: null })
    })

    it('reads the same for the tiles', () => {
        expect(appearanceOf(nord, 'light', false)).toBe('dark')
        expect(appearanceOf(catppuccin, 'light', true)).toBe('light')
        expect(appearanceOf(catppuccin, 'system', true)).toBe('dark')
    })
})

describe('Vizier', () => {
    it('paints nothing and has no cache', () => {
        expect(paintOf(vizier)).toBeNull()
        expect(resolve(vizier, 'system', true)).toEqual({ appearance: 'dark', dataTheme: null, vars: {} })
        expect(resolve(vizier, 'dark', false)).toEqual({ appearance: 'dark', dataTheme: 'dark', vars: {} })
    })
})

describe('Vizier Swapped', () => {
    it('paints only what it changes', () => {
        expect(Object.keys(resolve(swapped, 'dark', true).vars).sort()).toEqual(['--vz-panel', '--vz-shell'])
    })

    it('does not touch the colours faces are drawn from', () => {
        for (const appearance of ['light', 'dark'] as const) {
            const { vars } = resolve(swapped, appearance, false)
            for (const name of ['--vz-live', '--vz-ink', '--vz-text-soft', '--vz-chart-4', '--vz-chart-5', '--vz-border']) {
                expect(vars[name]).toBeUndefined()
            }
        }
    })
})

describe('paint cache', () => {
    it('holds only resolved variables', () => {
        const paint = paintOf(nord)!
        expect(paint.id).toBe('nord')
        expect(paint.light).toBeUndefined()
        expect(Object.keys(paint.dark!)).toHaveLength(50)
        for (const [name, value] of Object.entries(paint.dark!)) {
            expect(name).toMatch(/^--vz-[a-z0-9-]+$/)
            expect(value).toMatch(/^#[0-9a-f]{6}$/)
        }
    })

    it('stays small', () => {
        expect(JSON.stringify(paintOf(catppuccin)).length).toBeLessThan(5000)
    })

    it('round trips', () => {
        const paint = paintOf(catppuccin)!
        expect(readPaint(JSON.stringify(paint))).toEqual(paint)
        expect(resolvePaint(paint, 'dark', false).vars).toEqual(resolve(catppuccin, 'dark', false).vars)
    })

    it.each([null, '', '{oops', 'null', '[]', '{}', '{"id":5}', '{"id":"x"}', '{"id":"x","dark":3}'])('reads %s as no cache', (raw) => {
        expect(readPaint(raw)).toBeNull()
    })

    it('drops anything that is not a variable and a colour', () => {
        const raw = JSON.stringify({ id: 'x', dark: { '--vz-panel': '#000000', '--vz-text': 'red;background:url(x)', color: '#ffffff', '--vz-ink': 'url(javascript:alert(1))' } })
        expect(readPaint(raw)!.dark).toEqual({ '--vz-panel': '#000000' })
    })
})

describe('applying', () => {
    it('sets variables and the attribute, and clears what the new scheme lacks', () => {
        const el = fakeElement()
        applyTo(el, resolve(nord, 'system', false))
        expect(el.vars.size).toBe(50)
        expect(el.attrs.get('data-theme')).toBe('dark')
        applyTo(el, resolve(vizier, 'system', false))
        expect(el.vars.size).toBe(0)
        expect(el.attrs.has('data-theme')).toBe(false)
    })

    it('refuses a value that is not a colour', () => {
        const el = fakeElement()
        applyTo(el, { appearance: 'dark', dataTheme: null, vars: { '--vz-panel': 'red;x:y', 'not-a-var': '#000000' } })
        expect(el.vars.size).toBe(0)
    })
})

describe('pre-paint script', () => {
    function run(stored: Record<string, string>, deviceIsDark = false) {
        const el = fakeElement()
        const meta = fakeElement()
        const env = {
            document: {
                documentElement: el,
                querySelector: (s: string) => (s === 'meta[name="theme-color"]' ? meta : null),
            },
            localStorage: { getItem: (k: string) => stored[k] ?? null },
            matchMedia: () => ({ matches: deviceIsDark }),
        }
        new Function('document', 'localStorage', 'matchMedia', PRE_PAINT_SCRIPT)(env.document, env.localStorage, env.matchMedia)
        return { ...el, meta }
    }

    it('does nothing with nothing stored', () => {
        const el = run({})
        expect(el.vars.size).toBe(0)
        expect(el.attrs.size).toBe(0)
    })

    it('keeps an old light or dark choice with no migration', () => {
        expect(run({ 'vizier.theme': 'dark' }).attrs.get('data-theme')).toBe('dark')
    })

    it('forces the only variant', () => {
        const el = run({ 'vizier.theme': 'light', 'vizier.scheme.paint': JSON.stringify(paintOf(nord)) })
        expect(el.attrs.get('data-theme')).toBe('dark')
        expect(el.vars.get('--vz-panel')).toBe(nord.dark!.roles['bg/panel'])
    })

    it('with both, follows the stored mode, then the device', () => {
        const paint = JSON.stringify(paintOf(catppuccin))
        expect(run({ 'vizier.theme': 'light', 'vizier.scheme.paint': paint }, true).vars.get('--vz-panel')).toBe('#eff1f5')
        expect(run({ 'vizier.scheme.paint': paint }, true).vars.get('--vz-panel')).toBe(catppuccin.dark!.roles['bg/panel'])
        expect(run({ 'vizier.scheme.paint': paint }, false).vars.get('--vz-panel')).toBe('#eff1f5')
        expect(run({ 'vizier.scheme.paint': paint }, true).attrs.has('data-theme')).toBe(false)
    })

    it('never throws, and paints nothing, on damaged data', () => {
        for (const raw of ['{oops', 'null', '5', '"x"']) {
            const el = run({ 'vizier.scheme.paint': raw })
            expect(el.vars.size).toBe(0)
        }
    })

    it('paints the browser chrome to the stored scheme', () => {
        expect(run({ 'vizier.theme': 'dark' }).meta.attrs.get('content')).toBe('#0b0b0c')
        expect(run({ 'vizier.theme': 'light' }).meta.attrs.get('content')).toBe('#ffffff')
    })

    it('paints the browser chrome to the device when nothing is stored', () => {
        expect(run({}, true).meta.attrs.get('content')).toBe('#0b0b0c')
        expect(run({}, false).meta.attrs.get('content')).toBe('#ffffff')
    })

    it('paints the browser chrome to the variant a scheme forces', () => {
        const el = run({ 'vizier.theme': 'light', 'vizier.scheme.paint': JSON.stringify(paintOf(nord)) })
        expect(el.attrs.get('data-theme')).toBe('dark')
        expect(el.meta.attrs.get('content')).toBe('#0b0b0c')
    })

    it('skips bad names and values', () => {
        const el = run({ 'vizier.scheme.paint': JSON.stringify({ id: 'x', dark: { '--vz-panel': 'red;x:y', color: '#000000', '--vz-ink': '#010203' } }) })
        expect([...el.vars]).toEqual([['--vz-ink', '#010203']])
    })
})
