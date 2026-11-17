// Pure helpers of the `<vz-*>` elements: how a number is shown and what a field starts with.

/** A number as the page asked to show it. */
export function formatValue(value: unknown, format: unknown): string {
    if (value === null || value === undefined) return ''
    if (typeof value !== 'number') return String(value)
    switch (format) {
        case 'percent':
            return `${Math.round(value * 1000) / 10}%`
        case 'currency':
            return new Intl.NumberFormat(undefined, { style: 'currency', currency: 'USD' }).format(value)
        case 'bytes': {
            const units = ['B', 'KB', 'MB', 'GB', 'TB']
            let n = value
            let u = 0
            while (Math.abs(n) >= 1024 && u < units.length - 1) {
                n /= 1024
                u++
            }
            return `${Math.round(n * 10) / 10} ${units[u]}`
        }
        case 'duration': {
            const s = Math.round(value)
            return s >= 3600 ? `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m` : s >= 60 ? `${Math.floor(s / 60)}m ${s % 60}s` : `${s}s`
        }
        default:
            return new Intl.NumberFormat().format(value)
    }
}

export type FieldValue = string | number | boolean | ''

/** The value a field starts with, by kind, from its `value` attribute. */
export function initialValue(kind: string, raw: string | undefined, min: number | undefined): FieldValue {
    switch (kind) {
        case 'checkbox':
        case 'switch':
            return raw === 'true' || raw === ''
        case 'number':
            return raw === undefined || raw.trim() === '' || !Number.isFinite(Number(raw)) ? '' : Number(raw)
        case 'slider':
            return raw !== undefined && Number.isFinite(Number(raw)) ? Number(raw) : (min ?? 0)
        default:
            return raw ?? ''
    }
}

/** The elements' styles, without font faces: the frame loads no font files by URL. */
export function elementStyles(css: string): string {
    return css.replace(/@font-face\s*\{[^}]*\}/g, '')
}
