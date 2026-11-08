// Removes the directory part of absolute and home paths, keeping the base name (FR-012).
// Workspace-relative paths (`src/main.rs`) and URLs are left alone.
import { outsideData } from './redact'
import type { Removals } from './schema'

const ROOTS = 'home|root|Users|tmp|var|opt|etc|usr|mnt|srv|Volumes|private|media|workspace'
const BAD = String.raw`"'` + '`<>|*?'
// A directory name may hold single spaces between words when it is followed by a separator; the base name may not.
const unixDir = `[^\\s/\\\\${BAD}]+(?: [^\\s/\\\\${BAD}]+)*`
const unixBase = `[^\\s/\\\\${BAD}]+`
const winDir = `[^\\s/\\\\:${BAD}]+(?: [^\\s/\\\\:${BAD}]+)*`
const winBase = `[^\\s/\\\\:${BAD}]+`

const FILE_URL = /file:\/\/(?:localhost)?\/[^\s"'`<>)\]]+/g
const URL_SPAN = /(\b(?:https?|ftp|wss?):\/\/[^\s<>"'`)\]]*|\bwww\.[^\s<>"'`)\]]*|\bmailto:[^\s<>"'`)\]]*)/

const QUOTED = new RegExp(
    String.raw`(?<=["'` + '`' + String.raw`])(?:/(?:${ROOTS})/|~/|[A-Za-z]:[\\/]|\\\\)[^"'` + '`' + String.raw`\n]*(?=["'` + '`' + String.raw`])`,
    'g'
)
const WINDOWS = new RegExp(String.raw`(?<![A-Za-z0-9])[A-Za-z]:[\\/]+(?:${winDir}[\\/]+)*${winBase}`, 'g')
const UNC = new RegExp(String.raw`(?<![\\\w])\\{2,4}[A-Za-z0-9_.-]+(?:\\{1,2}${winDir})*\\{1,2}${winBase}`, 'g')
const UNIX = new RegExp(String.raw`(?<![\w./~@%-])/(?:${ROOTS})(?:/${unixDir})*/${unixBase}`, 'g')
const TILDE = new RegExp(String.raw`(?<![\w./~@%-])~(?:/${unixDir})*/${unixBase}`, 'g')

const TRAILING = /[.,;:!?)\]}]+$/

function baseOf(path: string): string {
    const slashless = path.replace(/[\\/]+$/, '')
    const tail = /[\\/]+$/.exec(path)?.[0] ?? ''
    const base = slashless.split(/[\\/]+/).pop() ?? ''
    return base + (base ? '' : '') + (tail && !base ? tail : '')
}

function replacer(counts: Removals, quoted: boolean) {
    return (match: string): string => {
        let path = match
        let tail = ''
        if (quoted) {
            const t = /\\+$/.exec(path)?.[0] ?? ''
            path = path.slice(0, path.length - t.length)
            tail = t
        } else {
            const t = TRAILING.exec(path)?.[0] ?? ''
            path = path.slice(0, path.length - t.length)
            tail = t
        }
        const base = baseOf(path)
        if (!base) return match
        counts.paths++
        return base + tail
    }
}

function strip(part: string, counts: Removals): string {
    let out = part.replace(FILE_URL, replacer(counts, false))
    const chunks = out.split(URL_SPAN)
    // Odd indices are URLs: untouched.
    out = chunks
        .map((chunk, i) => {
            if (i % 2 === 1) return chunk
            return chunk
                .replace(QUOTED, replacer(counts, true))
                .replace(UNC, replacer(counts, false))
                .replace(WINDOWS, replacer(counts, false))
                .replace(UNIX, replacer(counts, false))
                .replace(TILDE, replacer(counts, false))
        })
        .join('')
    return out
}

/** Removes the directory part of every path in `text`, adding the number removed to `counts.paths`. */
export function stripPaths(text: string, counts: Removals): string {
    if (!text || !/[\\/~]/.test(text)) return text
    return outsideData(text, (part) => strip(part, counts))
}
