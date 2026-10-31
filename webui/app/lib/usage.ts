import type { Activity, Breakdown, DailyUsage } from '../api/endpoints/usage'

export type Range = '7' | '30' | 'custom'

export const ACTIVITY_LABEL: Record<Activity, string> = {
    chat: 'Chat',
    project: 'Projects',
    background: 'Background',
    scheduled: 'Scheduled',
    dreams: 'Dreams',
    compaction: 'Compaction',
    vision: 'Vision',
    other: 'Other',
}

export const SERIES_COLOURS = ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5'] as const
export type Colour = (typeof SERIES_COLOURS)[number] | 'ink'

/** The `[start, end)` of a range ending at `now`. A custom range is whatever the person picked. */
export function rangeDates(range: Range, now: Date, custom?: { from: Date; to: Date }): { start: Date; end: Date } {
    if (range === 'custom' && custom) return { start: custom.from, end: custom.to }
    const days = range === '7' ? 7 : 30
    return { start: new Date(now.getTime() - days * 86_400_000), end: now }
}

/** The five biggest groups by value, then the rest folded into "Other". */
export function topSeries<T extends { key: string; label: string }>(
    items: T[],
    value: (item: T) => number,
    keep = 5
): { top: T[]; other: { count: number; value: number } | null } {
    const sorted = [...items].sort((a, b) => value(b) - value(a))
    const top = sorted.slice(0, keep)
    const rest = sorted.slice(keep)
    return { top, other: rest.length ? { count: rest.length, value: rest.reduce((n, i) => n + value(i), 0) } : null }
}

/** One stacked bar per day: the chosen measure per series key (the top series, and "other"). */
export function dailyStacks(
    daily: DailyUsage[],
    measure: 'cost' | 'tokens',
    keys: Activity[]
): { label?: string; values: Record<string, number> }[] {
    return daily.map((d, i) => {
        const values: Record<string, number> = {}
        for (const [activity, v] of Object.entries(d.by_activity) as [Activity, { cost_usd?: number | null; tokens: number }][]) {
            const n = measure === 'cost' ? (v.cost_usd ?? 0) : v.tokens
            const key = keys.includes(activity) ? activity : 'other'
            values[key] = (values[key] ?? 0) + n
        }
        const label = i % Math.ceil(daily.length / 7) === 0 ? new Date(`${d.date}T00:00:00`).toLocaleDateString('en-US', { month: 'short', day: 'numeric' }) : undefined
        return { label, values }
    })
}

/** Share of the largest row, 0–1, for a bar list. */
export function shareOf(rows: Breakdown[], row: Breakdown, measure: 'cost' | 'tokens'): number {
    const total = rows.reduce((n, r) => n + (measure === 'cost' ? (r.cost_usd ?? 0) : r.tokens), 0)
    const mine = measure === 'cost' ? (row.cost_usd ?? 0) : row.tokens
    return total > 0 ? mine / total : 0
}

/** "+12% vs previous 30 days", or nothing without a previous figure. */
export function versusPrevious(now: number | null | undefined, prev: number | null | undefined, label: string): string | undefined {
    if (now == null || prev == null || prev === 0) return undefined
    const pct = Math.round(((now - prev) / prev) * 100)
    return `${pct >= 0 ? '+' : ''}${pct}% vs ${label}`
}
