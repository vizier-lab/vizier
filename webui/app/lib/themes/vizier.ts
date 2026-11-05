// Vizier's two variants, read from the stylesheet so its values are written in one place only.
import tokensCss from '@vizier/design/tokens.css?raw'
import { KEY_COLORS, KEY_ROLE, roleOfVar, ROLES, type Appearance, type Hex, type Keys, type Role, type Variant } from './roles'

/** Everything like `--vz-name: #rrggbb;` inside one block. */
function colorsIn(block: string): Record<string, Hex> {
    return Object.fromEntries([...block.matchAll(/(--vz-[a-z0-9-]+):\s*(#[0-9a-fA-F]{3,8})\s*;/g)].map((m) => [m[1], m[2].toLowerCase()]))
}

function blockAfter(css: string, selector: string): string {
    const start = css.indexOf(selector)
    if (start < 0) return ''
    const open = css.indexOf('{', start)
    return css.slice(open + 1, css.indexOf('}', open))
}

/** The colour variables each appearance has. Dark inherits what it doesn't set, as the page does. */
export function parseTokens(css: string): Record<Appearance, Record<string, Hex>> {
    const light = colorsIn(blockAfter(css, ':root {'))
    return { light, dark: { ...light, ...colorsIn(blockAfter(css, ':root[data-theme="dark"] {')) } }
}

export function variantFromTokens(vars: Record<string, Hex>): Variant {
    const roles = {} as Record<Role, Hex>
    for (const [name, value] of Object.entries(vars)) {
        const role = roleOfVar(name)
        if (role) roles[role] = value
    }
    const keys = Object.fromEntries(KEY_COLORS.map((k) => [k, roles[KEY_ROLE[k]]])) as Keys
    return { keys, roles, overridden: [] }
}

const parsed = parseTokens(tokensCss)

export const VIZIER_TOKENS_CSS = tokensCss
export const VIZIER_LIGHT = variantFromTokens(parsed.light)
export const VIZIER_DARK = variantFromTokens(parsed.dark)
export const vizierVariant = (appearance: Appearance): Variant => (appearance === 'dark' ? VIZIER_DARK : VIZIER_LIGHT)

/** Whether a variant has a value for every role; a test holds Vizier's two to it. */
export const isComplete = (variant: Variant) => ROLES.every((r) => /^#[0-9a-f]{6}$/.test(variant.roles[r] ?? ''))
