// Removes the directory part of absolute and home paths, keeping the base name (FR-012).
// Workspace-relative paths (`src/main.rs`) and URLs are left alone.
import { outsideData } from './redact'
import type { Removals } from './schema'

const ROOTS = 'home|root|Users|tmp|var|opt|etc|usr|mnt|srv|Volumes|private|media|workspace|proc|sys|dev|run|data|app|Library|Applications|System|lib|lib64|bin|sbin|boot|snap|nix'
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
// `~/x/y` and `~alice/x/y`.
const TILDE = new RegExp(String.raw`(?<![\w./~@%-])~(?:[A-Za-z_][\w.-]*)?(?:/${unixDir})*/${unixBase}`, 'g')
// `$HOME/x`, `${HOME}/x`, `%USERPROFILE%\x`, `%APPDATA%\x`.
const ENV_HOME = new RegExp(
    String.raw`(?<![\w$%])(?:\$HOME|\$\{HOME\}|\$USERPROFILE|\$\{USERPROFILE\}|%(?:USERPROFILE|HOMEPATH|HOMEDRIVE|APPDATA|LOCALAPPDATA|TEMP|TMP)%)(?:[\\/]+${winDir})*[\\/]+${winBase}`,
    'g'
)
// Win32 namespaces: `\\?\C:\Users\alice\a.txt`, `\\?\UNC\srv\share\a.txt`, `\\.\pipe\x`.
const WIN_NS = new RegExp(String.raw`(?<![\\\w])\\\\[?.]\\(?:UNC\\|[A-Za-z]:\\+)?(?:${winDir}\\+)*${winBase}`, 'g')
// A user's home directory by itself says who they are and nothing else: `/home/alice`, `C:\Users\alice`.
const HOME_ONLY = /^(?:\/(?:home|Users)\/[^/\\]+|[A-Za-z]:[\\/]+Users[\\/]+[^\\/]+|~[^/\\]+)[\\/]*$/
// Pseudo-devices every shell script uses; they name nothing about the machine.
const HARMLESS = /^\/dev\/(?:null|zero|full|random|urandom|tty|stdin|stdout|stderr)$/

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
        if (HARMLESS.test(path)) return match
        if (HOME_ONLY.test(path.replace(/^file:\/\/(?:localhost)?\/?(?=[A-Za-z]:)/, '').replace(/^file:\/\/(?:localhost)?/, ''))) {
            counts.paths++
            return '~' + tail
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
                .replace(WIN_NS, replacer(counts, false))
                .replace(ENV_HOME, replacer(counts, false))
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
