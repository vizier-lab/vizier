// The purposes a colour serves, the few colours a person edits first, and the shape of a scheme.
// Names are the design names, because they are what a person reads in an exported theme file.

export type RoleGroup = 'bg' | 'line' | 'dot' | 'text' | 'fill' | 'state' | 'data' | 'code'

const ROLE_TABLE = [
    ['bg/ground', '--vz-ground', 'Ground'],
    ['bg/sidebar', '--vz-sidebar', 'Side panels'],
    ['bg/shell', '--vz-shell', 'Frame'],
    ['bg/panel', '--vz-panel', 'Content'],
    ['bg/surface', '--vz-surface', 'Cards'],
    ['bg/raised', '--vz-raised', 'Raised'],
    ['bg/pressed', '--vz-pressed', 'Pressed'],
    ['line/border', '--vz-border', 'Border'],
    ['line/border-strong', '--vz-border-strong', 'Strong border'],
    ['line/focus', '--vz-focus', 'Focus ring'],
    ['line/border-hover', '--vz-border-hover', 'Border on hover'],
    ['dot/unlit', '--vz-unlit', 'Unlit dot'],
    ['dot/off', '--vz-dot-off', 'Off dot'],
    ['text/faint', '--vz-text-faint', 'Faint text'],
    ['text/muted', '--vz-text-muted', 'Muted text'],
    ['text/soft', '--vz-text-soft', 'Soft text and plain code'],
    ['text/default', '--vz-text', 'Text'],
    ['fill/ink', '--vz-ink', 'Accent'],
    ['fill/ink-hover', '--vz-ink-hover', 'Accent on hover'],
    ['fill/ink-pressed', '--vz-ink-pressed', 'Accent pressed'],
    ['state/live', '--vz-live', 'Live'],
    ['state/live-text', '--vz-live-text', 'Live text'],
    ['state/live-bg', '--vz-live-bg', 'Live ground'],
    ['state/rank', '--vz-rank', 'Rank'],
    ['state/rank-text', '--vz-rank-text', 'Rank text'],
    ['state/rank-bg', '--vz-rank-bg', 'Rank ground'],
    ['state/failed', '--vz-failed', 'Failed'],
    ['state/failed-text', '--vz-failed-text', 'Failed text'],
    ['state/failed-bg', '--vz-failed-bg', 'Failed ground'],
    ['state/failed-line', '--vz-failed-line', 'Failed edge'],
    ['state/failed-hover', '--vz-failed-hover', 'Failed on hover'],
    ['state/failed-pressed', '--vz-failed-pressed', 'Failed pressed'],
    ['state/retrying', '--vz-retrying', 'Retrying'],
    ['state/retrying-bg', '--vz-retrying-bg', 'Retrying ground'],
    ['state/retrying-line', '--vz-retrying-line', 'Retrying edge'],
    ['state/retrying-text', '--vz-retrying-text', 'Retrying text'],
    ['state/selection', '--vz-selection', 'Selected text ground'],
    ['state/selection-text', '--vz-selection-text', 'Selected text letters'],
    ['data/graph-edge', '--vz-graph-edge', 'Graph link'],
    ['data/graph-edge-strong', '--vz-graph-edge-strong', 'Graph link, selected'],
    ['data/chart-1', '--vz-chart-1', 'Chart 1'],
    ['data/chart-2', '--vz-chart-2', 'Chart 2'],
    ['data/chart-3', '--vz-chart-3', 'Chart 3'],
    ['data/chart-4', '--vz-chart-4', 'Chart 4'],
    ['data/chart-5', '--vz-chart-5', 'Chart 5'],
    ['code/keyword', '--vz-code-keyword', 'Keyword'],
    ['code/string', '--vz-code-string', 'String'],
    ['code/number', '--vz-code-number', 'Number'],
    ['code/comment', '--vz-code-comment', 'Comment'],
    ['code/function', '--vz-code-function', 'Function'],
    ['code/punctuation', '--vz-code-punctuation', 'Punctuation'],
] as const

export type Role = (typeof ROLE_TABLE)[number][0]
export type Hex = string

export const ROLES: readonly Role[] = ROLE_TABLE.map((r) => r[0])
export const ROLE_VAR = Object.fromEntries(ROLE_TABLE.map((r) => [r[0], r[1]])) as Record<Role, string>
export const ROLE_LABEL = Object.fromEntries(ROLE_TABLE.map((r) => [r[0], r[2]])) as Record<Role, string>
export const ROLE_VARS: ReadonlySet<string> = new Set(ROLE_TABLE.map((r) => r[1]))
const ROLE_SET: ReadonlySet<string> = new Set(ROLES)

export const isRole = (name: string): name is Role => ROLE_SET.has(name)

/** The role a design system variable belongs to. */
export const roleOfVar = (cssVar: string): Role | undefined => ROLE_TABLE.find((r) => r[1] === cssVar)?.[0]

export const groupOf = (role: Role): RoleGroup => role.split('/')[0] as RoleGroup

/** The roles of one group, in the order the design system lists them. */
export const rolesIn = (group: RoleGroup): Role[] => ROLES.filter((r) => groupOf(r) === group)

export const ROLE_GROUPS: { group: RoleGroup; label: string }[] = [
    { group: 'bg', label: 'Backgrounds' },
    { group: 'line', label: 'Lines' },
    { group: 'dot', label: 'Dots' },
    { group: 'text', label: 'Text' },
    { group: 'fill', label: 'Accent' },
    { group: 'state', label: 'Status' },
    { group: 'data', label: 'Charts and graphs' },
    { group: 'code', label: 'Code' },
]

export const KEY_COLORS = ['content', 'frame', 'text', 'accent', 'live', 'retrying', 'failed'] as const
export type KeyColor = (typeof KEY_COLORS)[number]
export type Keys = Record<KeyColor, Hex>

/** The role each key colour is, so a key can be read back from a variant's roles. */
export const KEY_ROLE: Record<KeyColor, Role> = {
    content: 'bg/panel',
    frame: 'bg/shell',
    text: 'text/default',
    accent: 'fill/ink',
    live: 'state/live',
    retrying: 'state/retrying',
    failed: 'state/failed',
}

export const KEY_LABEL: Record<KeyColor, string> = {
    content: 'Content',
    frame: 'Sidebar',
    text: 'Text',
    accent: 'Accent',
    live: 'Live',
    retrying: 'Retrying',
    failed: 'Failed',
}

export type Appearance = 'light' | 'dark'
export type Mode = 'system' | Appearance

export type Variant = {
    keys: Keys
    /** Always complete in storage, so reading needs no derivation. */
    roles: Record<Role, Hex>
    /** Roles that keep their value when a key changes. */
    overridden: Role[]
}

export type Scheme = {
    /** A preset's slug, or `custom-` and a uuid. */
    id: string
    name: string
    kind: 'preset' | 'custom'
    light?: Variant
    dark?: Variant
    /**
     * Only for presets built on the stylesheet: the roles actually painted over it. Vizier paints
     * none, and Vizier Swapped only what it changes, so faces keep their fixed colours under both.
     */
    paints?: Partial<Record<Appearance, Role[]>>
    /** Shown under a preset's name in place of what its variants say. */
    detail?: string
}

export const DEFAULT_SCHEME_ID = 'vizier'
export const MAX_CUSTOM_SCHEMES = 50
export const MAX_NAME_LENGTH = 40
