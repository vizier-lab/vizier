// lint-tokens-exempt: pins values read from tokens.css.
import { describe, expect, it } from 'vitest'
import { ROLES, ROLE_VAR, ROLE_VARS } from './roles'
import { isComplete, parseTokens, VIZIER_DARK, VIZIER_LIGHT, VIZIER_TOKENS_CSS } from './vizier'

describe('tokens.css and the roles', () => {
    it('has exactly 51 roles', () => {
        expect(ROLES).toHaveLength(51)
        expect(new Set(ROLES).size).toBe(51)
        expect(new Set(Object.values(ROLE_VAR)).size).toBe(51)
    })

    // A new colour token that isn't a role would silently escape every scheme.
    it('every colour variable is a role', () => {
        const css = VIZIER_TOKENS_CSS.replace(/\/\*[\s\S]*?\*\//g, '')
        const variables = [...css.matchAll(/(--vz-[a-z0-9-]+):\s*#[0-9a-fA-F]/g)].map((m) => m[1])
        expect(variables.length).toBeGreaterThan(50)
        expect([...new Set(variables)].filter((v) => !ROLE_VARS.has(v))).toEqual([])
    })

    it('reads a complete variant for each appearance', () => {
        expect(isComplete(VIZIER_LIGHT)).toBe(true)
        expect(isComplete(VIZIER_DARK)).toBe(true)
    })

    it('reads the same value the stylesheet has', () => {
        expect(VIZIER_LIGHT.roles['bg/shell']).toBe('#f1f1f3')
        expect(VIZIER_DARK.roles['bg/shell']).toBe('#070708')
        expect(VIZIER_LIGHT.roles['code/string']).toBe('#046c4e')
        expect(VIZIER_DARK.roles['code/string']).toBe('#10b981')
    })

    it('takes its keys from the roles', () => {
        expect(VIZIER_LIGHT.keys).toEqual({ content: '#ffffff', frame: '#f1f1f3', text: '#111114', accent: '#111114', live: '#059669', retrying: '#a86a12', failed: '#c8371d' })
    })

    it('lets dark inherit what it does not set', () => {
        expect(VIZIER_DARK.roles['state/selection']).toBe(VIZIER_LIGHT.roles['state/selection'])
    })

    it('parses with either line ending', () => {
        const css = ':root {\r\n  --vz-panel: #ffffff; /* bg/panel */\r\n}\r\n:root[data-theme="dark"] {\r\n  --vz-panel: #000000;\r\n}'
        expect(parseTokens(css)).toEqual({ light: { '--vz-panel': '#ffffff' }, dark: { '--vz-panel': '#000000' } })
    })
})
