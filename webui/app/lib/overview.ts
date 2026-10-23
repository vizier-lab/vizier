import type { ActivityRow as GridRow } from '@vizier/design'
import type { Activity, Overview } from '../api/endpoints/overview'
import { cost } from './format'

const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString('en-US')} ${n === 1 ? one : many}`

/** "48 agents at court · 2 live · 1 failed · ~$4.12 today". */
export function greetingMeta(o: Overview): string {
    const parts = [`${plural(o.agents.total, 'agent')} at court`]
    if (o.agents.live) parts.push(`${o.agents.live} live`)
    if (o.agents.failed) parts.push(`${o.agents.failed} failed`)
    if (o.cost_today_usd > 0) parts.push(`${cost(o.cost_today_usd)} today`)
    return parts.join(' · ')
}

/** The tooltip of every hour's dot: "14:00 · 3 turns", in the person's own hours. */
export function hourHints(hours: string[], turns: number[]): string[] {
    return hours.map((h, i) => {
        const n = turns[i] ?? 0
        const at = new Date(h)
        const label = `${String(at.getHours()).padStart(2, '0')}:00`
        return `${label} · ${n === 0 ? 'no turns' : plural(n, 'turn')}`
    })
}

/** The grid's rows: the "All N" row first when there is one, then the agents. */
export function gridRows(a: Activity): GridRow[] {
    const row = (r: Activity['rows'][number], total = false): GridRow => ({
        label: r.label,
        hours: r.turns,
        live: r.live,
        total,
        hints: hourHints(a.hours, r.turns),
    })
    return [...(a.all ? [row(a.all, true)] : []), ...a.rows.map((r) => row(r))]
}

/** Right of the grid's title. */
export function gridMeta(a: Activity): string | undefined {
    if (a.agent_total === 0) return undefined
    const live = [...a.rows, ...(a.all ? [a.all] : [])].find((r) => r.agent_id === null)?.live
    if (a.all) return `all agents + ${a.rows.length} most active${live ? ' · some live now' : ''} · dot size = turns per hour`
    if (a.agent_total === 1) {
        const total = a.rows[0].turns.reduce((s, n) => s + n, 0)
        return `${plural(total, 'turn')} in 24 hours`
    }
    return `${plural(a.agent_total, 'agent')} · dot size = turns per hour`
}

/** One agent: what the day was, in words. */
export function summaryLine(a: Activity): string | undefined {
    const row = a.rows[0]
    const kinds = a.today_by_kind
    if (a.agent_total !== 1 || !row || !kinds) return undefined
    const parts = [plural(kinds.chat, 'chat turn'), plural(kinds.scheduled, 'scheduled task'), plural(kinds.dream_stages, 'dream stage')]
    if (kinds.background) parts.push(plural(kinds.background, 'background turn'))
    const list = parts.length > 1 ? `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}` : parts[0]
    return `${row.label} is ${row.live ? 'replying now' : 'idle'}. ${list} today.`
}

/** "21:04" today, "yesterday", then a short date. */
export function feedTime(at: string, now: Date = new Date()): string {
    const d = new Date(at)
    const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
    const days = Math.round((day(now) - day(d)) / 86_400_000)
    if (days <= 0) return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
    if (days === 1) return 'yesterday'
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}
