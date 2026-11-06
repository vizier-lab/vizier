// The instance's timezone. Every time on screen is shown on the instance's clock, whatever zone
// the browser is in, so a person and an agent read the same "09:00".
//
// `zoned` is the one conversion: it returns a Date whose browser-local fields are the instance's
// wall clock, so calendar arithmetic, getters and `toLocale*` work unchanged on its result. Such a
// Date is for reading its fields; it is not the real instant, so keep the original for comparing
// moments or sending them anywhere.

export interface InstanceZone {
    /** IANA name, or null when the host's zone can't be named. */
    name: string | null
    /** The offset at load time, such as +07:00. Used only when there is no name. */
    offset: string
}

let current: InstanceZone | null = null
let formatter: Intl.DateTimeFormat | null = null

export function setInstanceZone(zone: InstanceZone | null) {
    current = zone
    formatter = null
    if (zone?.name) {
        try {
            formatter = new Intl.DateTimeFormat('en-US', {
                timeZone: zone.name,
                hourCycle: 'h23',
                era: 'short',
                year: 'numeric',
                month: 'numeric',
                day: 'numeric',
                hour: 'numeric',
                minute: 'numeric',
                second: 'numeric',
            })
        } catch {
            console.warn(`The browser doesn't know the timezone ${zone.name}; using its offset`)
        }
    }
}

export const instanceZone = () => current

function offsetMinutes(offset: string): number {
    const m = /^([+-])(\d{2}):(\d{2})$/.exec(offset)
    if (!m) return 0
    const minutes = Number(m[2]) * 60 + Number(m[3])
    return m[1] === '-' ? -minutes : minutes
}

/**
 * A Date whose browser-local fields are the instance's wall clock at this instant. With no zone
 * loaded it's the browser's own time. A wall-clock time the browser's own zone skips (one hour a
 * year, where it has daylight saving) reads an hour late, because a Date can't hold it.
 */
export function zoned(instant: string | number | Date): Date {
    const real = new Date(instant)
    if (!current || Number.isNaN(real.getTime())) return real
    if (formatter) {
        const parts: Record<string, string> = {}
        for (const part of formatter.formatToParts(real)) parts[part.type] = part.value
        const year = parts.era?.startsWith('B') ? 1 - Number(parts.year) : Number(parts.year)
        const at = new Date(2000, 0, 1)
        at.setFullYear(year, Number(parts.month) - 1, Number(parts.day))
        at.setHours(Number(parts.hour), Number(parts.minute), Number(parts.second), real.getMilliseconds())
        return at
    }
    const shifted = new Date(real.getTime() + offsetMinutes(current.offset) * 60_000)
    const at = new Date(2000, 0, 1)
    at.setFullYear(shifted.getUTCFullYear(), shifted.getUTCMonth(), shifted.getUTCDate())
    at.setHours(shifted.getUTCHours(), shifted.getUTCMinutes(), shifted.getUTCSeconds(), shifted.getUTCMilliseconds())
    return at
}

const pad = (n: number) => String(n).padStart(2, '0')

/**
 * The zone-less `YYYY-MM-DDTHH:mm:ss` of a Date's browser-local fields, which is what a form holds
 * after `zoned`. The server reads it as local time and resolves gaps and overlaps.
 */
export function wallClockString(date: Date): string {
    const year = String(date.getFullYear()).padStart(4, '0')
    return `${year}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}
