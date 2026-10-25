export { middleTruncate } from '@vizier/design'

const en = new Intl.NumberFormat('en-US')

/** 1,234,567 */
export function count(n: number): string {
    return en.format(n)
}

/** Chart axes only: 61,880 → 61.9k. Everywhere else shows the full number. */
export function abbreviate(n: number): string {
    const abs = Math.abs(n)
    if (abs >= 1e9) return `${trim(n / 1e9)}B`
    if (abs >= 1e6) return `${trim(n / 1e6)}M`
    if (abs >= 1e3) return `${trim(n / 1e3)}k`
    return String(n)
}

function trim(n: number): string {
    return n.toFixed(1).replace(/\.0$/, '')
}

/** ~$0.03, ~$4.12; sub-cent amounts say so rather than showing zero. */
export function cost(usd: number): string {
    if (usd <= 0) return '$0'
    if (usd < 0.01) return '<$0.01'
    return `~$${usd.toFixed(2)}`
}

/** "2m 14s", "18.4s", "1h 05m" */
export function duration(ms: number): string {
    if (ms < 0) ms = 0
    if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
    const s = Math.round(ms / 1000)
    if (s < 3600) return `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, '0')}s`
    return `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, '0')}m`
}

/** "just now", "5m ago", "2h ago", "3d ago", then a date. */
export function relativeTime(date: string | number | Date, now: number = Date.now()): string {
    const t = new Date(date).getTime()
    const diff = now - t
    if (diff < 45_000) return 'just now'
    const m = Math.round(diff / 60_000)
    if (m < 60) return `${m}m ago`
    const h = Math.round(diff / 3_600_000)
    if (h < 24) return `${h}h ago`
    const d = Math.round(diff / 86_400_000)
    if (d < 7) return `${d}d ago`
    return absoluteTime(t, true)
}

/** "Oct 23, 14:05", or just the date when `dateOnly`. */
export function absoluteTime(date: string | number | Date, dateOnly = false): string {
    const d = new Date(date)
    const day = d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
    if (dateOnly) return day
    const time = d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
    return `${day}, ${time}`
}

/** "21:04" today, "yesterday", then a short date. */
export function feedTime(at: string, now: Date = new Date()): string {
    const d = new Date(at)
    const day = (x: Date) =>
        new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
    const days = Math.round((day(now) - day(d)) / 86_400_000)
    if (days <= 0)
        return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
    if (days === 1) return 'yesterday'
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/** 8.2 GB, 512 MB, 900 B — download sizes, where "GB" is GiB. */
export function bytes(n: number): string {
    if (!Number.isFinite(n) || n <= 0) return '0 B'
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    let i = 0
    let v = n
    while (v >= 1024 && i < units.length - 1) {
        v /= 1024
        i += 1
    }
    return `${v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`
}
