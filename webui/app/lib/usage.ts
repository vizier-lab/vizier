import type { Activity, Breakdown, DailyUsage, MediaCall, PriceSource, Units, UsageKind } from '../api/endpoints/usage'
import { abbreviate, cost, count } from './format'

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
    consultation: 'Consultations',
    indexing: 'Indexing',
}

export const KIND_LABEL: Record<UsageKind, string> = {
    text: 'Text',
    image: 'Images',
    speech: 'Speech',
    transcription: 'Transcription',
    embedding: 'Embeddings',
}

/** "2.5 h", "4 min", "38 s": a length of audio. */
export function audioLength(seconds: number): string {
    if (seconds >= 3600) return `${(seconds / 3600).toFixed(1).replace(/\.0$/, '')} h`
    if (seconds >= 60) return `${Math.round(seconds / 60)} min`
    return `${Math.round(seconds * 10) / 10} s`
}

/** A media amount in its own words: "233 images", "140k chars · 2.5 h audio", "41M tokens". */
export function formatUnits(units?: Units | null): string | undefined {
    if (!units) return undefined
    const parts: string[] = []
    if (units.images != null) parts.push(`${count(units.images)} image${units.images === 1 ? '' : 's'}`)
    if (units.characters != null) parts.push(`${abbreviate(units.characters)} chars`)
    if (units.audio_seconds != null) parts.push(`${audioLength(units.audio_seconds)} audio`)
    if (units.tokens != null) parts.push(`${abbreviate(units.tokens)} embedding tokens`)
    return parts.length ? parts.join(' · ') : undefined
}

/** How a priced figure is labelled: free for a local model, a note for the shipped table. */
export function priceNote(source?: PriceSource): string | undefined {
    if (source === 'local') return 'free · local'
    if (source === 'curated') return "estimated from Vizier's price table"
    if (source === 'none') return 'unpriced'
    return undefined
}

/** One line for a chat step's media call: "openai / gpt-image-1 · 1 image (1024x1024) · ~$0.17". */
export function mediaCallLine(call: MediaCall): string {
    const amount = formatUnits(call.units)
    const variant = call.variant ? ` (${call.variant})` : ''
    const price = call.local ? 'free · local' : call.cost_usd != null ? cost(call.cost_usd) : 'unpriced'
    return [`${call.provider} / ${call.model}`, amount ? `${amount}${variant}` : KIND_LABEL[call.kind], price].join(' · ')
}

/** Unpriced media amounts, in their own words, for the "not in the estimate" note. */
export function unpricedMedia(units?: Partial<Record<UsageKind, Units>>): string | undefined {
    if (!units) return undefined
    const parts = Object.values(units).map(formatUnits).filter((p): p is string => !!p)
    return parts.length ? parts.join(', ') : undefined
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
