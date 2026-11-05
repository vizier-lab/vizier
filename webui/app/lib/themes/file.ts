// lint-tokens-exempt: an error message shows an example colour.
// The theme file: a public, hand-editable format (contracts/theme-file.md), so later versions must keep
// reading version 1. Only a name and colours ever come out of a file, whatever else it holds.
import { z } from 'zod'
import { isHex } from './color'
import { completeKeys, distinctName, knownColors, newCustomId } from './customs'
import { derive } from './derive'
import { KEY_COLORS, MAX_CUSTOM_SCHEMES, MAX_NAME_LENGTH, ROLES, type Appearance, type Hex, type Role, type Scheme, type Variant } from './roles'

const FORMAT = 'vizier-theme'
const VERSION = 1
const MAX_BYTES = 256 * 1024

export const THEME_FILE_SUFFIX = '.vizier-theme.json'

const NAME = z.string().trim().min(1).max(MAX_NAME_LENGTH)

export function serialize(scheme: Scheme): string {
    const part = (variant?: Variant) =>
        variant && {
            keys: Object.fromEntries(KEY_COLORS.map((key) => [key, variant.keys[key]])),
            roles: Object.fromEntries(ROLES.map((role) => [role, variant.roles[role]])),
        }
    return `${JSON.stringify({ format: FORMAT, version: VERSION, name: scheme.name, light: part(scheme.light), dark: part(scheme.dark) }, null, 2)}\n`
}

export function fileNameFor(name: string): string {
    const slug = name
        .toLowerCase()
        .normalize('NFD')
        .replace(/[̀-ͯ]/g, '')
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-|-$/g, '')
    return `${slug || 'theme'}${THEME_FILE_SUFFIX}`
}

export type ParseResult = { ok: true; scheme: Scheme } | { ok: false; error: string }

const refuse = (error: string): ParseResult => ({ ok: false, error })
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/** The first thing that isn't a colour, by the path a person would look for in their file. */
function badColor(appearance: Appearance, part: 'keys' | 'roles', raw: unknown): string | null {
    if (raw === undefined) return null
    if (!isObject(raw)) return `${appearance}.${part} isn't a list of colors.`
    for (const name of part === 'keys' ? KEY_COLORS : ROLES) {
        if (Object.hasOwn(raw, name) && !isHex(raw[name])) return `${appearance}.${part}.${name} isn't a color. Use a value like #1e1e2e.`
    }
    return null
}

function variantFrom(raw: Record<string, unknown>, appearance: Appearance): Variant {
    const givenRoles = knownColors((raw.roles as Record<string, unknown>) ?? {}, ROLES) as Partial<Record<Role, Hex>>
    const keys = completeKeys(knownColors((raw.keys as Record<string, unknown>) ?? {}, KEY_COLORS), givenRoles, appearance)
    const given = Object.keys(givenRoles) as Role[]
    // A file with every role is an untouched export, or close to it: nothing there is a local edit.
    return { keys, roles: { ...derive(keys), ...givenRoles }, overridden: given.length === ROLES.length ? [] : given }
}

/** Checked in the order the contract gives; the first failure refuses the whole file. */
export function parse(text: string, existing: Scheme[]): ParseResult {
    if (new TextEncoder().encode(text).length > MAX_BYTES) return refuse('That file is too large to be a theme.')
    let file: unknown
    try {
        file = JSON.parse(text)
    } catch {
        return refuse("That file isn't valid JSON.")
    }
    if (!isObject(file) || file.format !== FORMAT) return refuse("That file isn't a Vizier theme.")
    if (file.version !== VERSION) return refuse('That theme needs a newer version of Vizier.')
    const name = NAME.safeParse(file.name)
    if (!name.success) return refuse(`Enter a theme name of 1 to ${MAX_NAME_LENGTH} characters.`)
    const light = isObject(file.light) ? file.light : undefined
    const dark = isObject(file.dark) ? file.dark : undefined
    if (!light && !dark) return refuse('That theme has no light or dark colors.')
    for (const [appearance, raw] of [
        ['light', light],
        ['dark', dark],
    ] as const) {
        if (!raw) continue
        const problem = badColor(appearance, 'keys', raw.keys) ?? badColor(appearance, 'roles', raw.roles)
        if (problem) return refuse(problem)
    }
    if (existing.filter((s) => s.kind === 'custom').length >= MAX_CUSTOM_SCHEMES) return refuse(`You've reached ${MAX_CUSTOM_SCHEMES} themes. Delete one to import this.`)
    return {
        ok: true,
        scheme: {
            id: newCustomId(),
            name: distinctName(name.data, existing.map((s) => s.name)),
            kind: 'custom',
            ...(light && { light: variantFrom(light, 'light') }),
            ...(dark && { dark: variantFrom(dark, 'dark') }),
        },
    }
}
