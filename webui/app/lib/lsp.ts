// The language-server panel's pure logic (spec 054): how the servers of a project group by
// language, what the strip item shows, and how an override form reads and writes.
import type { LspOverride, LspServer, LspSnapshot, LspView } from '../api/endpoints/projects'

export type Dot = 'pulse' | 'warn' | 'fail'

/** One language in the panel: an override applies to all of its roots, so they share a header. */
export type LanguageGroup = {
    id: string
    name: string
    /** The command that runs: the override's, else the built-in one when it is known. */
    command: string
    overridden: boolean
    /** Every server of this language is missing: one note, not a row per root. */
    missing: { install_hint: string; reason: string } | null
    roots: LspServer[]
}

/** Group servers by language, in the order the server sent them. */
export function groupByLanguage(view: Pick<LspView, 'servers' | 'languages' | 'overrides'>): LanguageGroup[] {
    const order: string[] = []
    const byLanguage = new Map<string, LspServer[]>()
    for (const server of view.servers) {
        if (!byLanguage.has(server.language)) {
            byLanguage.set(server.language, [])
            order.push(server.language)
        }
        byLanguage.get(server.language)!.push(server)
    }
    return order.map((id) => {
        const servers = byLanguage.get(id)!
        const override = view.overrides?.find((o) => o.language === id)
        const known = view.languages?.find((l) => l.id === id)
        const allMissing = servers.every((s) => s.state === 'missing')
        const first = servers[0]
        return {
            id,
            name: first.name,
            command: override?.command ?? known?.default_command ?? '',
            overridden: servers.some((s) => s.overridden) || !!override,
            missing: allMissing
                ? { install_hint: first.install_hint ?? '', reason: first.reason ?? '' }
                : null,
            roots: allMissing ? [] : servers,
        }
    })
}

/** How many servers are ready: the strip's number. */
export function readyCount(snapshot: Pick<LspSnapshot, 'servers'>): number {
    return snapshot.servers.filter((s) => s.state === 'ready').length
}

/** The worst thing going on, as the strip's dot. */
export function worstDot(snapshot: Pick<LspSnapshot, 'enabled' | 'servers'>): Dot | undefined {
    if (!snapshot.enabled) return undefined
    const has = (state: LspServer['state']) => snapshot.servers.some((s) => s.state === state)
    if (has('failed')) return 'fail'
    if (has('missing')) return 'warn'
    if (has('starting')) return 'pulse'
    return undefined
}

/** The panel's right-hand line: "2 ready · 1 starting". Missing counts languages, not roots. */
export function metaLine(snapshot: Pick<LspSnapshot, 'enabled' | 'servers'>): string {
    if (!snapshot.enabled) return 'off'
    const count = (state: LspServer['state']) => snapshot.servers.filter((s) => s.state === state).length
    const missing = new Set(snapshot.servers.filter((s) => s.state === 'missing').map((s) => s.language)).size
    const parts = [
        count('ready') && `${count('ready')} ready`,
        count('starting') && `${count('starting')} starting`,
        missing && `${missing} missing`,
        count('failed') && `${count('failed')} failed`,
    ].filter(Boolean)
    return parts.join(' · ') || 'none running'
}

/** A root as a person reads it: the folder itself is `./`, anything else ends in a slash. */
export function rootLabel(root: string): string {
    return root === '.' ? './' : `${root.replace(/\/$/, '')}/`
}

/** The strip item's accessible label. */
export function stripLabel(snapshot: Pick<LspSnapshot, 'enabled' | 'servers'>): string {
    return snapshot.enabled ? 'language servers ready' : 'language servers off'
}

// ------------------------------------------------------------------------------ override form

/** `KEY=value` lines (or comma separated) to the env map, or an error naming the bad entry. */
export function parseEnv(text: string): { env: Record<string, string> } | { error: string } {
    const env: Record<string, string> = {}
    for (const raw of text.split(/[\n,]/)) {
        const entry = raw.trim()
        if (!entry) continue
        const at = entry.indexOf('=')
        const name = at < 0 ? entry : entry.slice(0, at).trim()
        if (at < 0 || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
            return { error: `"${entry}" is not NAME=value` }
        }
        env[name] = entry.slice(at + 1)
    }
    if (Object.keys(env).length > 32) return { error: 'At most 32 variables' }
    return { env }
}

export function formatEnv(env: Record<string, string>): string {
    return Object.entries(env)
        .map(([k, v]) => `${k}=${v}`)
        .join(' ')
}

/**
 * Split an arguments field the way a shell would for the simple cases: spaces separate, quotes
 * group. No expansion, since nothing runs through a shell.
 */
export function parseArgs(text: string): string[] {
    const args: string[] = []
    let current = ''
    let quote: '"' | "'" | null = null
    let started = false
    for (const ch of text) {
        if (quote) {
            if (ch === quote) quote = null
            else current += ch
        } else if (ch === '"' || ch === "'") {
            quote = ch
            started = true
        } else if (/\s/.test(ch)) {
            if (started || current) args.push(current)
            current = ''
            started = false
        } else {
            current += ch
        }
    }
    if (started || current) args.push(current)
    return args
}

export function formatArgs(args: string[]): string {
    return args.map((a) => (/[\s"']/.test(a) ? JSON.stringify(a) : a)).join(' ')
}

/** The override a form holds, or the reason it cannot be saved. */
export function buildOverride(
    command: string,
    argsText: string,
    envText: string
): { override: LspOverride } | { error: string } {
    const trimmed = command.trim()
    if (!trimmed) return { error: 'Enter a command' }
    if (/[\r\n]/.test(trimmed)) return { error: 'The command is one line' }
    const env = parseEnv(envText)
    if ('error' in env) return env
    const args = parseArgs(argsText)
    if (args.length > 32) return { error: 'At most 32 arguments' }
    return { override: { command: trimmed, args, env: env.env } }
}
