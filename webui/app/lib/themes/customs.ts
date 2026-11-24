// The custom schemes kept in this browser: reading them without trusting them, and the rules on names.
import { z } from 'zod'
import { isHex } from './color'
import { derive } from './derive'
import { isRole, KEY_COLORS, KEY_ROLE, MAX_NAME_LENGTH, ROLES, type Appearance, type Hex, type Keys, type Role, type Scheme, type Variant } from './roles'
import { vizierVariant } from './vizier'

const STORED_VARIANT = z.object({ keys: z.record(z.string(), z.unknown()).optional(), roles: z.record(z.string(), z.unknown()).optional(), overridden: z.array(z.unknown()).optional() })

const STORED_SCHEME = z.object({
    id: z.string().regex(/^custom-[\w-]+$/),
    name: z.string().trim().min(1).max(MAX_NAME_LENGTH),
    light: STORED_VARIANT.optional(),
    dark: STORED_VARIANT.optional(),
})

/** Only known names are ever copied out, so nothing like `__proto__` is read into an object. */
export function knownColors(raw: Record<string, unknown>, names: readonly string[]): Record<string, Hex> {
    const out: Record<string, Hex> = {}
    for (const name of names) {
        if (!Object.hasOwn(raw, name)) continue
        const value = raw[name]
        if (isHex(value)) out[name] = value.toLowerCase()
    }
    return out
}

/** A missing key takes its own role's colour if there is one, otherwise Vizier's for that appearance. */
export function completeKeys(keys: Record<string, Hex>, roles: Partial<Record<Role, Hex>>, appearance: Appearance): Keys {
    const vizier = vizierVariant(appearance).keys
    return Object.fromEntries(KEY_COLORS.map((key) => [key, keys[key] ?? roles[KEY_ROLE[key]] ?? vizier[key]])) as Keys
}

function storedVariant(raw: z.infer<typeof STORED_VARIANT>, appearance: Appearance): Variant {
    const roles = knownColors(raw.roles ?? {}, ROLES) as Partial<Record<Role, Hex>>
    const keys = completeKeys(knownColors(raw.keys ?? {}, KEY_COLORS), roles, appearance)
    // A role an earlier version didn't have comes from the keys, so an old scheme keeps loading.
    return { keys, roles: { ...derive(keys), ...roles }, overridden: (raw.overridden ?? []).filter((r): r is Role => typeof r === 'string' && isRole(r)) }
}

/** Whatever could be read. One damaged scheme is dropped; the others stay. */
export function parseCustoms(raw: string | null): Scheme[] {
    if (!raw) return []
    let list: unknown
    try {
        list = (JSON.parse(raw) as { schemes?: unknown } | null)?.schemes
    } catch {
        return []
    }
    if (!Array.isArray(list)) return []
    const schemes: Scheme[] = []
    for (const entry of list) {
        const parsed = STORED_SCHEME.safeParse(entry)
        if (!parsed.success || (!parsed.data.light && !parsed.data.dark)) continue
        const { id, name, light, dark } = parsed.data
        if (schemes.some((s) => s.id === id)) continue
        schemes.push({ id, name, kind: 'custom', ...(light && { light: storedVariant(light, 'light') }), ...(dark && { dark: storedVariant(dark, 'dark') }) })
    }
    return schemes
}

export { distinctName, nameProblem, newCustomId, serializeCustoms } from './names'
