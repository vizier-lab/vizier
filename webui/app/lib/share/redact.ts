// Removes credentials from text before it leaves (FR-011). Pattern table, one marker `[removed: <what>]`.
// The goal is no false positive on ordinary conversation (git SHAs, UUIDs, base64 images, the word
// "token") and no miss on a key split across lines or sitting inside a JSON string. Pure functions.
import type { Removals } from './schema'

export const newCounts = (): Removals => ({ credentials: 0, paths: 0, recall: 0, reasoning: 0, media: 0 })
export const marker = (what: string) => `[removed: ${what}]`

/** A base64 body or data URI is opaque data: never scanned (a lookalike inside it is not a secret). */
const DATA_URI = /data:[A-Za-z0-9.+-]+\/[A-Za-z0-9.+-]+(?:;[A-Za-z0-9=._-]+)*;base64,[A-Za-z0-9+/=_-]+/g
const OPAQUE = /^[A-Za-z0-9+/=_-]{256,}$/

/** Applies `fn` to the parts of `text` that are not opaque data. */
export function outsideData(text: string, fn: (part: string) => string): string {
    if (OPAQUE.test(text)) return text
    if (!text.includes('base64,')) return fn(text)
    let out = ''
    let last = 0
    for (const m of text.matchAll(DATA_URI)) {
        out += fn(text.slice(last, m.index)) + m[0]
        last = (m.index ?? 0) + m[0].length
    }
    return out + fn(text.slice(last))
}

// A line break: a real one, or an escaped one (`\n`), or one escaped twice when JSON sits inside a JSON string (`\\n`).
const BR = String.raw`(?:\r?\n|\\+(?:r\\+)?n)`
const WORD = '[A-Za-z0-9_-]'
const NOT_BEFORE = String.raw`(?<![A-Za-z0-9_-])`
const NOT_AFTER = String.raw`(?![A-Za-z0-9_-])`
const looksRandom = (s: string) => /\d/.test(s) || (/[a-z]/.test(s) && /[A-Z]/.test(s))

interface Rule {
    label: string
    re: RegExp
    /** Returns the replacement for a match, or null to leave it. */
    accept?: (m: RegExpExecArray) => string | null
}

/** `prefix` then a run of body characters of at least `min`, possibly continued after a line break. */
function keyRule(label: string, prefix: string, min: number, extra = ''): Rule {
    const re = new RegExp(`${NOT_BEFORE}(${prefix})(${WORD}{2,})(?:(${BR}[ \\t>]*)(${WORD}{6,}))?`, 'g')
    return {
        label,
        re,
        accept: (m) => {
            const [, , body, brk, cont] = m
            let total = body.length
            let tail = ''
            if (cont !== undefined && body.length < min && looksRandom(cont)) total += cont.length
            else if (cont !== undefined) tail = brk + cont
            if (total < min) return null
            if (extra === 'digit' && !/\d/.test(body + (tail ? '' : (cont ?? '')))) return null
            return marker(label) + tail
        },
    }
}

// A limit is not a secret: `max_token=2048`, `maxToken: 4096`, `prompt_token=…`.
const NOT_A_LIMIT = String.raw`(?<!(?:max|min|num|total|prompt|completion|context|input|output)[_-]?)`
const SECRET_NAME = String.raw`(?:password|passwd|passphrase|pwd|secret|api[_-]?key|access[_-]?key|private[_-]?key|${NOT_A_LIMIT}token|credentials?)`
const STOP = new Set(['true', 'false', 'null', 'none', 'undefined', 'string', 'required', 'yes', 'no', 'empty', 'redacted'])

const RULES: Rule[] = [
    // A private key block, to its end marker or to the end of the text when it is cut off.
    {
        label: 'private key',
        re: /-----BEGIN [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----[\s\S]*?(?:-----END [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----|$)/g,
        accept: () => marker('private key'),
    },
    keyRule('key', 'sk-(?:ant|proj|svcacct|admin)-', 16),
    keyRule('key', 'sk-', 32, 'digit'),
    keyRule('key', 'AIza', 35),
    {
        label: 'key',
        re: new RegExp(`(?<![A-Za-z0-9+/_-])(?:AKIA|ASIA|AGPA|AIDA|AROA|ANPA|ANVA)[A-Z0-9]{16}(?![A-Za-z0-9+/=_-])`, 'g'),
        accept: () => marker('key'),
    },
    keyRule('token', 'gh[pousr]_', 36),
    keyRule('token', 'github_pat_', 22),
    keyRule('token', 'xox[abprs]-', 10),
    keyRule('key', '(?:sk|rk)_(?:live|test)_', 16),
    keyRule('token', 'npm_', 36),
    keyRule('token', 'hf_', 30),
    keyRule('token', 'glpat-', 20),
    keyRule('token', 'ya29\\.', 20),
    keyRule('key', 'GOCSPX-', 20),
    {
        // SendGrid: `SG.<22>.<43>`.
        label: 'key',
        re: new RegExp(`${NOT_BEFORE}SG\\.[A-Za-z0-9_-]{16,}\\.[A-Za-z0-9_-]{16,}`, 'g'),
        accept: () => marker('key'),
    },
    {
        // Twilio API key (`SK`) and account (`AC`) SIDs: 32 hex digits.
        label: 'key',
        re: new RegExp(`${NOT_BEFORE}(?:SK|AC)[0-9a-f]{32}${NOT_AFTER}`, 'g'),
        accept: () => marker('key'),
    },
    {
        // The secret is the path after `/services/` (Slack) or `/webhooks/<id>/` (Discord).
        label: 'token',
        re: /(hooks\.slack\.com\/services\/|discord(?:app)?\.com\/api\/webhooks\/)([A-Za-z0-9/_-]{8,})/gi,
        accept: (m) => (m[2].startsWith('[removed') ? null : m[1] + marker('token')),
    },
    {
        // Azure storage / service bus connection strings.
        label: 'key',
        re: /((?:Account|SharedAccess)Key=)([^;\s"'\\]{12,})/gi,
        accept: (m) => m[1] + marker('key'),
    },
    {
        // A `Cookie:` or `Set-Cookie:` header: name=value pairs up to the end of the line (or of the JSON string).
        label: 'cookie',
        re: new RegExp(`${NOT_BEFORE}((?:Set-)?Cookie\\\\?["']?\\s*[:=]\\s*)([^\\r\\n"\\\\]+)`, 'gi'),
        accept: (m) => (/[A-Za-z0-9_.-]+=/.test(m[2]) && !m[2].startsWith('[removed') ? m[1] + marker('cookie') : null),
    },
    keyRule('key', 'whsec_', 16),
    {
        label: 'token',
        re: new RegExp(`${NOT_BEFORE}eyJ[A-Za-z0-9_-]{8,}\\.eyJ[A-Za-z0-9_-]{8,}\\.[A-Za-z0-9_-]*`, 'g'),
        accept: () => marker('token'),
    },
    {
        label: 'token',
        re: /(Authorization\\?["']?\s*[:=]\s*\\?["']?(?:Bearer|Basic|Token|Digest)\s+)([^\s"'\\,;]{8,})/gi,
        accept: (m) => (m[2].startsWith('[removed') ? null : m[1] + marker('token')),
    },
    {
        label: 'token',
        re: /(?<![A-Za-z])(Bearer\s+)([A-Za-z0-9._~+/=-]{20,})/g,
        accept: (m) => (/\d/.test(m[2]) ? m[1] + marker('token') : null),
    },
    {
        label: 'password',
        // The password runs to the LAST `@` before the host (a password may hold an unescaped `@`).
        re: /(:\/\/[^\s/:@"'<>\\]*:)([^\s/"'<>\\]{3,})(@)(?=[^\s@/"'<>\\]+(?:[:/?#]|[\s"'<>\\]|$))/g,
        accept: (m) => (m[2].startsWith('[removed') ? null : m[1] + marker('password') + m[3]),
    },
    {
        label: 'key',
        re: /([?&](?:key|sig|signature|auth)=)(?!\[removed)([^&\s#"'\\]{6,})/gi,
        accept: (m) => m[1] + marker('key'),
    },
    // `password=…`, `API_KEY=…`, `"token": "…"`, `password: …` (this last only when it looks like a value).
    {
        label: 'secret',
        re: new RegExp(`(${SECRET_NAME})(\\\\?["']?)(\\s*[:=]\\s*)(\\\\?["']?)(?!\\[removed)([^\\s"'\\\\&,;<>)]{4,})`, 'gi'),
        accept: (m) => {
            const [, name, q1, sep, q2, value] = m
            if (STOP.has(value.toLowerCase())) return null
            const strong = sep.includes('=') || q1 !== '' || q2 !== '' || /[\d\W_]/.test(value)
            if (!strong) return null
            const what = /^(?:pass|pwd)/i.test(name) ? 'password' : /^token/i.test(name) ? 'token' : 'secret'
            return name + q1 + sep + q2 + marker(what)
        },
    },
]

/** Removes credentials from `text`, adding the number removed to `counts.credentials`. */
export function redactText(text: string, counts: Removals): string {
    if (!text) return text
    return outsideData(text, (part) => {
        let out = part
        for (const rule of RULES) {
            out = out.replace(rule.re, (...args) => {
                const groups = args.slice(0, -2) as string[]
                const m = Object.assign(groups, { index: args[args.length - 2] as number, input: out }) as unknown as RegExpExecArray
                const replacement = rule.accept ? rule.accept(m) : marker(rule.label)
                if (replacement === null) return m[0]
                counts.credentials++
                return replacement
            })
        }
        return out
    })
}

/** Object keys whose string value is a credential whatever it looks like. */
const SECRET_KEY = /(?:^|[_-])(?:password|passwd|passphrase|pwd|secret|token|api[_-]?key|apikey|access[_-]?key|private[_-]?key|authorization|credentials?|bearer)$/i
const MAX_DEPTH = 256

/** A copy of `value` with every string cleaned and every string under a secret-looking key replaced. Never mutates, never throws on cycles or depth. */
export function redactValue(value: unknown, counts: Removals, key?: string, seen: WeakSet<object> = new WeakSet(), depth = 0): unknown {
    if (typeof value === 'string') {
        if (key && SECRET_KEY.test(key) && value !== '' && !value.startsWith('[removed')) {
            counts.credentials++
            return marker('key')
        }
        return redactText(value, counts)
    }
    if (value === null || typeof value !== 'object') return value
    if (depth > MAX_DEPTH) return '[truncated]'
    if (seen.has(value)) return '[circular]'
    seen.add(value)
    const out = Array.isArray(value)
        ? value.map((v) => redactValue(v, counts, undefined, seen, depth + 1))
        : Object.fromEntries(Object.entries(value).map(([k, v]) => [redactText(k, counts), redactValue(v, counts, k, seen, depth + 1)]))
    seen.delete(value)
    return out
}
