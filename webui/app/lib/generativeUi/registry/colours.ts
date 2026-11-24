// lint-tokens-exempt: a neutral grey when a theme token cannot be read (WebGL needs a concrete colour).
// The closed colour list of the catalogue (`ink`, `accent`, `series-1…6`, …) or `#rrggbb`, as the
// design tokens they stand for. SVG takes the CSS variable; WebGL needs the resolved value.

const VARS: Record<string, string> = {
    ink: '--vz-ink',
    muted: '--vz-text-muted',
    accent: '--vz-rank',
    ok: '--vz-live',
    warn: '--vz-retrying',
    error: '--vz-failed',
    surface: '--vz-surface',
    'series-1': '--vz-chart-1',
    'series-2': '--vz-chart-2',
    'series-3': '--vz-chart-3',
    'series-4': '--vz-chart-4',
    'series-5': '--vz-chart-5',
    'series-6': '--vz-text-faint',
}

const HEX = /^#[0-9a-fA-F]{6}$/

/** A value usable in CSS (`fill`, `stroke`, `background`): `var(--vz-…)` for a token, hex as is. */
export function cssColour(value: unknown, fallback = 'ink'): string {
    if (typeof value === 'string' && HEX.test(value)) return value
    const token = typeof value === 'string' && value in VARS ? value : fallback
    return `var(${VARS[token]})`
}

/** The same as a concrete `#rrggbb`, for WebGL, read from the page's current theme. */
export function resolvedColour(value: unknown, fallback = 'ink'): string {
    if (typeof value === 'string' && HEX.test(value)) return value
    const token = typeof value === 'string' && value in VARS ? value : fallback
    const computed = getComputedStyle(document.documentElement).getPropertyValue(VARS[token]).trim()
    return HEX.test(computed) ? computed : '#808080'
}
