const DAYS = ['Sundays', 'Mondays', 'Tuesdays', 'Wednesdays', 'Thursdays', 'Fridays', 'Saturdays']

const clock = (h: string, m: string) => `${h.padStart(2, '0')}:${m.padStart(2, '0')}`
const num = (s: string) => /^\d+$/.test(s)

/**
 * A cron expression in words for the usual shapes ("Daily at 09:00", "Mondays 09:00", "Every 6
 * hours"), or null for anything stranger, which the caller shows as written.
 */
export function describeCron(expression: string): string | null {
    const parts = expression.trim().split(/\s+/)
    // croner takes an optional leading seconds field.
    const [m, h, dom, mon, dow] = parts.length === 6 ? parts.slice(1) : parts
    if (parts.length < 5 || parts.length > 6) return null
    if (dom === '*' && mon === '*') {
        if (num(m) && num(h) && dow === '*') return `Daily at ${clock(h, m)}`
        if (num(m) && num(h) && num(dow)) return `${DAYS[Number(dow) % 7] ?? dow} ${clock(h, m)}`
        if (num(m) && /^\*\/\d+$/.test(h) && dow === '*') return `Every ${h.slice(2)} hours`
        if (m === '*' && h === '*' && dow === '*') return 'Every minute'
        if (/^\*\/\d+$/.test(m) && h === '*' && dow === '*') return `Every ${m.slice(2)} minutes`
        if (num(m) && h === '*' && dow === '*') return `Hourly at :${m.padStart(2, '0')}`
    }
    if (num(m) && num(h) && num(dom) && mon === '*' && dow === '*') return `Monthly on day ${dom} at ${clock(h, m)}`
    return null
}
