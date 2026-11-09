// The colour pairs a variant has to keep legible. The editor warns about them; presets must meet them.
import { contrast, deltaEok } from './color'
import { ROLE_LABEL, type Hex, type Role } from './roles'

type Pair = { fg: Role; bg: Role; min: number; label: string }

const pair = (fg: Role, bg: Role, min: number, label: string): Pair => ({ fg, bg, min, label })

export const CONTRAST_PAIRS: Pair[] = [
    pair('text/default', 'bg/panel', 4.5, 'Text on content'),
    pair('text/muted', 'bg/panel', 4.5, 'Muted text on content'),
    pair('text/faint', 'bg/panel', 4.5, 'Faint text on content'),
    pair('text/default', 'bg/surface', 4.5, 'Text on cards'),
    pair('bg/ground', 'fill/ink', 4.5, 'Primary button text'),
    pair('bg/ground', 'state/failed', 4.5, 'Destructive button text'),
    pair('state/live-text', 'bg/panel', 4.5, 'Live text on content'),
    pair('state/failed-text', 'bg/panel', 4.5, 'Failed text on content'),
    pair('code/keyword', 'bg/surface', 4.5, 'Code keywords'),
    pair('code/string', 'bg/surface', 4.5, 'Code strings'),
    pair('code/number', 'bg/surface', 4.5, 'Code numbers'),
    pair('code/function', 'bg/surface', 4.5, 'Code functions'),
    pair('code/punctuation', 'bg/surface', 4.5, 'Code punctuation'),
    pair('text/soft', 'bg/surface', 4.5, 'Plain code'),
    pair('code/comment', 'bg/surface', 3, 'Code comments'),
    pair('line/focus', 'bg/panel', 3, 'Focus ring'),
    pair('state/live', 'bg/panel', 3, 'Live mark'),
    pair('state/failed', 'bg/panel', 3, 'Failed mark'),
    pair('state/retrying', 'bg/panel', 3, 'Retrying mark'),
    pair('state/retrying-text', 'bg/panel', 4.5, 'Retrying text on content'),
]

/** Accent and live closer than this hide the live dot on a sigil, which differs by colour alone. */
export const MIN_ACCENT_LIVE_DISTANCE = 0.1

export type ColorProblem = { id: string; roles: [Role, Role]; message: string }

export function colorProblems(roles: Record<Role, Hex>): ColorProblem[] {
    const problems: ColorProblem[] = []
    for (const { fg, bg, min, label } of CONTRAST_PAIRS) {
        const ratio = contrast(roles[fg], roles[bg])
        if (ratio < min) {
            problems.push({ id: `${fg}|${bg}`, roles: [fg, bg], message: `${label} is ${ratio.toFixed(1)}:1, below ${min}:1` })
        }
    }
    if (deltaEok(roles['fill/ink'], roles['state/live']) < MIN_ACCENT_LIVE_DISTANCE) {
        problems.push({
            id: 'accent-live',
            roles: ['fill/ink', 'state/live'],
            message: `${ROLE_LABEL['fill/ink']} and ${ROLE_LABEL['state/live']} are too alike, so a sigil's live dot would disappear`,
        })
    }
    return problems
}
