// What a link in a generated interface may be: https, http or mailto, nothing else. Parsed with
// `URL`, so scheme tricks (`java\tscript:`, mixed case, leading control characters) fail rather
// than pass; credentials in the address and protocol-relative `//host` are refused too. The
// host is shown beside a label that does not say it, so a label cannot hide the destination.
import { LIMITS } from './limits'

export const ALLOWED_SCHEMES = ['https', 'http', 'mailto'] as const

export type ParsedLink = { href: string; host: string }

// Whitespace and control characters never belong in an address; `URL` would quietly drop some.
// eslint-disable-next-line no-control-regex
const UNSAFE = /[\u0000-\u0020\u007f-\u009f\u2028\u2029]/

export function parseHref(raw: unknown): ParsedLink | null {
    if (typeof raw !== 'string' || raw === '' || raw.length > LIMITS.hrefLength || UNSAFE.test(raw)) return null
    let url: URL
    try {
        url = new URL(raw)
    } catch {
        return null
    }
    const scheme = url.protocol.slice(0, -1)
    if (!(ALLOWED_SCHEMES as readonly string[]).includes(scheme)) return null
    if (url.username || url.password) return null
    if (scheme === 'mailto') {
        const address = decodeURIComponent(url.pathname)
        const at = address.lastIndexOf('@')
        return at > 0 && at < address.length - 1 ? { href: url.href, host: address.slice(at + 1) } : null
    }
    return url.hostname ? { href: url.href, host: url.hostname } : null
}

/** The host to show beside `label`, or null when the label already says where the link goes. */
export function hostToDisclose(label: string, link: ParsedLink): string | null {
    const shown = label.trim().toLowerCase().replace(/^https?:\/\//, '').replace(/\/$/, '')
    const host = link.host.toLowerCase()
    return shown === host || shown === link.href.toLowerCase().replace(/^https?:\/\//, '').replace(/\/$/, '') || shown === host.replace(/^www\./, '') ? null : link.host
}

/** The markdown links in some text, as written. */
export function markdownLinks(text: string): string[] {
    return [...text.matchAll(/\]\(\s*<?([^)\s>]*)/g)].map((m) => m[1])
}
