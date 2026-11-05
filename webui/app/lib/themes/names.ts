// Names and storing custom schemes: light enough for the store, which every page loads.
import { nanoid } from 'nanoid'
import { MAX_NAME_LENGTH, type Scheme } from './roles'

export function serializeCustoms(schemes: Scheme[]): string {
    return JSON.stringify({ version: 1, schemes: schemes.map(({ id, name, light, dark }) => ({ id, name, light, dark })) })
}

export const newCustomId = () => `custom-${nanoid()}`

/** What is wrong with a name, or null. Names are unique among all schemes, ignoring case. */
export function nameProblem(name: string, schemes: Scheme[], ownId?: string): string | null {
    const trimmed = name.trim()
    if (!trimmed || trimmed.length > MAX_NAME_LENGTH) return `Enter a theme name of 1 to ${MAX_NAME_LENGTH} characters.`
    const lower = trimmed.toLowerCase()
    if (schemes.some((s) => s.id !== ownId && s.name.toLowerCase() === lower)) return 'That name is already taken. Try another.'
    return null
}

/** The name, or the nearest free one: `Catppuccin (2)`, shortened so the suffix fits the limit. */
export function distinctName(name: string, taken: string[]): string {
    const used = new Set(taken.map((n) => n.toLowerCase()))
    const base = name.trim()
    if (!used.has(base.toLowerCase())) return base
    for (let n = 2; ; n++) {
        const suffix = ` (${n})`
        const candidate = base.slice(0, MAX_NAME_LENGTH - suffix.length).trimEnd() + suffix
        if (!used.has(candidate.toLowerCase())) return candidate
    }
}
