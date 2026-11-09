// lint-tokens-exempt: pure colour logic; white and black are the only literals, for contrast.
// From a few key colours to every role, and the edits a person can make to a variant.
import { ensureContrast, hexToOklch, isAchromatic, mix, rotateHue, withLightness, contrast } from './color'
import { KEY_COLORS, KEY_ROLE, type Hex, type KeyColor, type Keys, type Role, ROLES, type Scheme, type Variant, type Appearance } from './roles'

// Rank and charts need a hue of their own even when the accent is a grey.
const MIN_HUE_CHROMA = 0.09
const NEUTRAL_HUE_BASE = '#5b7be8'

/** Which keys each role follows. A key change recomputes only the roles that list it. */
const DEPENDS_ON: Record<string, readonly KeyColor[]> = {
    'bg/panel': ['content'],
    'bg/shell': ['frame'],
    'text/default': ['text'],
    'fill/ink': ['accent'],
    'state/live': ['live'],
    'state/retrying': ['retrying'],
    'state/failed': ['failed'],
    'bg/sidebar': ['frame', 'content'],
    'fill/ink-hover': ['accent', 'content'],
    'fill/ink-pressed': ['accent', 'content'],
    'state/rank': ['accent', 'content', 'text'],
    'state/rank-text': ['accent', 'content', 'text'],
    'state/rank-bg': ['accent', 'content', 'text'],
    'data/chart-1': ['accent', 'content', 'text'],
    'data/chart-2': ['accent', 'content', 'text'],
    'data/chart-3': ['accent', 'content', 'text'],
    'data/chart-4': ['accent', 'content', 'text'],
    'data/chart-5': ['accent', 'content', 'text'],
    'state/live-text': ['live', 'content'],
    'state/live-bg': ['live', 'content'],
    'state/failed-text': ['failed', 'content'],
    'state/failed-bg': ['failed', 'content'],
    'state/failed-line': ['failed', 'content'],
    'state/failed-hover': ['failed', 'content', 'text'],
    'state/failed-pressed': ['failed', 'content', 'text'],
    'state/retrying-text': ['retrying', 'content'],
    'state/retrying-bg': ['retrying', 'content'],
    'state/retrying-line': ['retrying', 'content'],
    'state/selection': ['live'],
    'state/selection-text': ['live'],
    // Code follows the accent and the text, never a status colour, so changing one leaves the other.
    'code/keyword': ['accent', 'content', 'text'],
    'code/string': ['accent', 'content', 'text'],
    'code/number': ['accent', 'content', 'text'],
    'code/function': ['text', 'content'],
    'code/punctuation': ['text', 'content'],
    'code/comment': ['text', 'content'],
}
// Everything else is a step between the content and the text.
export const dependsOn = (role: Role): readonly KeyColor[] => DEPENDS_ON[role] ?? ['content', 'text']

export function derive(keys: Keys): Record<Role, Hex> {
    const { content, frame, text, accent, live, retrying, failed } = keys
    const toward = (weight: number) => mix(content, text, weight)
    const away = (weight: number) => mix(text, content, weight)
    const dark = hexToOklch(content).l < hexToOklch(text).l

    const surface = toward(0.05)
    const muted = ensureContrast(away(0.28), content, 4.5)
    const faint = ensureContrast(away(0.42), content, 4.5)

    // Rank and charts turn the accent's hue, at a lightness that reads on this content.
    const hueSource = isAchromatic(accent) ? NEUTRAL_HUE_BASE : accent
    const series = (degrees: number) => withLightness(rotateHue(hueSource, degrees, MIN_HUE_CHROMA), dark ? 0.72 : 0.58)
    const rank = series(30)

    // A grey accent keeps code in shades of grey; a coloured one lends it two more hues.
    const codeHue = (degrees: number) => (isAchromatic(accent) ? accent : rotateHue(accent, degrees))

    const selection = ensureContrast(live, '#ffffff', 4.5)
    const failedStep = (weight: number) => withLightness(failed, hexToOklch(failed).l + (dark ? weight : -weight))

    return {
        'bg/ground': content,
        'bg/sidebar': mix(frame, content, 0.5),
        'bg/shell': frame,
        'bg/panel': content,
        'bg/surface': surface,
        'bg/raised': toward(0.1),
        'bg/pressed': toward(0.16),
        'line/border': toward(0.12),
        'line/border-strong': toward(0.2),
        'line/focus': ensureContrast(away(0.15), content, 3),
        'line/border-hover': toward(0.38),
        'dot/unlit': toward(0.2),
        'dot/off': toward(0.12),
        'text/faint': faint,
        'text/muted': muted,
        'text/soft': ensureContrast(away(0.12), surface, 4.5),
        'text/default': text,
        'fill/ink': accent,
        'fill/ink-hover': mix(accent, content, 0.12),
        'fill/ink-pressed': mix(accent, content, 0.24),
        'state/live': live,
        'state/live-text': ensureContrast(live, content, 4.5),
        'state/live-bg': mix(content, live, 0.12),
        'state/rank': rank,
        'state/rank-text': ensureContrast(rank, content, 4.5),
        'state/rank-bg': mix(content, rank, 0.14),
        'state/failed': failed,
        'state/failed-text': ensureContrast(failed, content, 4.5),
        'state/failed-bg': mix(content, failed, 0.1),
        'state/failed-line': mix(content, failed, 0.3),
        'state/failed-hover': failedStep(0.05),
        'state/failed-pressed': failedStep(-0.05),
        'state/retrying': retrying,
        'state/retrying-text': ensureContrast(retrying, content, 4.5),
        'state/retrying-bg': mix(content, retrying, 0.1),
        'state/retrying-line': mix(content, retrying, 0.3),
        'state/selection': selection,
        'state/selection-text': contrast('#ffffff', selection) >= contrast('#000000', selection) ? '#ffffff' : '#000000',
        'data/graph-edge': toward(0.15),
        'data/graph-edge-strong': toward(0.35),
        'data/chart-1': series(70),
        'data/chart-2': series(140),
        'data/chart-3': series(200),
        'data/chart-4': series(260),
        'data/chart-5': series(320),
        'code/keyword': ensureContrast(accent, surface, 4.5),
        'code/string': ensureContrast(codeHue(120), surface, 4.5),
        'code/number': ensureContrast(codeHue(-60), surface, 4.5),
        'code/comment': ensureContrast(away(0.5), surface, 3),
        'code/function': ensureContrast(text, surface, 4.5),
        'code/punctuation': ensureContrast(muted, surface, 4.5),
    }
}

/** A variant from its keys, with any roles a preset or a file gave of its own. */
export function makeVariant(keys: Keys, given: Partial<Record<Role, Hex>> = {}, overridden: Role[] = []): Variant {
    return { keys, roles: { ...derive(keys), ...given }, overridden }
}

/** Changes one key: that key's role follows it, and so do dependents unless they are overridden. */
export function setKey(variant: Variant, key: KeyColor, hex: Hex): Variant {
    const keys = { ...variant.keys, [key]: hex }
    const derived = derive(keys)
    const roles = { ...variant.roles }
    for (const role of ROLES) {
        if (dependsOn(role).includes(key) && !variant.overridden.includes(role)) roles[role] = derived[role]
    }
    return { ...variant, keys, roles }
}

/** Sets one role by hand. Its value now holds when a key changes. A key's own role moves the key. */
export function overrideRole(variant: Variant, role: Role, hex: Hex): Variant {
    const key = KEY_COLORS.find((k) => KEY_ROLE[k] === role)
    const overridden = variant.overridden.includes(role) ? variant.overridden : [...variant.overridden, role]
    return { ...variant, keys: key ? { ...variant.keys, [key]: hex } : variant.keys, roles: { ...variant.roles, [role]: hex }, overridden }
}

/** Back to the value the keys give. */
export function resetRole(variant: Variant, role: Role): Variant {
    return { ...variant, roles: { ...variant.roles, [role]: derive(variant.keys)[role] }, overridden: variant.overridden.filter((r) => r !== role) }
}

/** A copy looks identical; `overridden` starts empty, so a later key change re-derives its dependents. */
export function duplicateVariant(variant: Variant): Variant {
    return { keys: { ...variant.keys }, roles: { ...variant.roles }, overridden: [] }
}

/** The missing variant, from the base's keys with the accent and live of the variant it has. */
export function addVariant(scheme: Scheme, appearance: Appearance, base: Keys): Scheme {
    if (scheme[appearance]) return scheme
    const other = scheme[appearance === 'light' ? 'dark' : 'light']
    const keys = other ? { ...base, accent: other.keys.accent, live: other.keys.live } : base
    return { ...scheme, [appearance]: makeVariant(keys) }
}

/** Removes a variant, unless it is the last one left. */
export function removeVariant(scheme: Scheme, appearance: Appearance): Scheme {
    if (!scheme[appearance === 'light' ? 'dark' : 'light']) return scheme
    const { [appearance]: _gone, ...rest } = scheme
    return rest
}
