/**
 * The `[[kind:name]]` entity wiki-link grammar (specs/066, contracts/link-grammar.md).
 * Kinds are reserved words: a `[[...]]` whose pre-colon word is one of these is an entity
 * link; every other `[[...]]` keeps its memory meaning and passes through untouched.
 */

import { createContext } from 'react'

export const ENTITY_KINDS = ['artifact', 'function', 'skill', 'task'] as const

export type EntityKind = (typeof ENTITY_KINDS)[number]

/** `encodeURIComponent` plus the parentheses it leaves alone: the href goes into a markdown
 * link's `( )`, which an unbalanced one in the name would end early. */
const enc = (name: string) => encodeURIComponent(name).replace(/\(/g, '%28').replace(/\)/g, '%29')

/** The single kind→target table (research R5). Names are agent-scoped by context. */
export function entityHref(agentId: string, kind: EntityKind, name: string): string {
    switch (kind) {
        case 'artifact':
            return `/${agentId}/artifacts?artifact=${enc(name)}`
        case 'function':
            return `/${agentId}/functions/${enc(name)}`
        case 'skill':
            return `/${agentId}/skills/${enc(name)}`
        case 'task':
            return `/${agentId}/tasks/${enc(name)}`
    }
}

/** Read a kind back out of an internal entity href, or null for any other URL. */
export function entityFromHref(href: string): { kind: EntityKind; name: string } | null {
    const q = href.indexOf('?')
    const path = q === -1 ? href : href.slice(0, q)
    const parts = path.split('/').filter(Boolean)
    if (parts.length === 2 && parts[1] === 'artifacts') {
        const name = new URLSearchParams(q === -1 ? '' : href.slice(q + 1)).get('artifact')
        return name ? { kind: 'artifact', name } : null
    }
    if (parts.length !== 3) return null
    const kind =
        parts[1] === 'functions' ? 'function' : parts[1] === 'skills' ? 'skill' : parts[1] === 'tasks' ? 'task' : null
    if (!kind) return null
    return { kind, name: decodeURIComponent(parts[2]) }
}

export type ParsedEntityLink = { kind: EntityKind; name: string; alias?: string }

/** Parse one `[[...]]` target; null unless it is a well-formed entity link. Whitespace
 * around the kind, name and alias is tolerated — models emit `[[artifact: report]]`. */
export function parseEntityLink(target: string): ParsedEntityLink | null {
    const trimmed = target.trim()
    const colon = trimmed.indexOf(':')
    if (colon <= 0) return null
    const kind = trimmed.slice(0, colon).trim()
    if (!ENTITY_KINDS.includes(kind as EntityKind)) return null
    const rest = trimmed.slice(colon + 1)
    const pipe = rest.indexOf('|')
    const name = (pipe === -1 ? rest : rest.slice(0, pipe)).trim()
    if (!name) return null
    const alias = pipe === -1 ? undefined : rest.slice(pipe + 1).trim() || undefined
    return { kind: kind as EntityKind, name, alias }
}

const LINK_RE = /\[\[([^\]\n|]+?)(?:\|([^\]\n]+))?\]\]/g

/**
 * Rewrite entity links into ordinary markdown links (label = alias or name), leaving
 * fenced/inline code and every non-entity `[[...]]` (the memory forms) untouched.
 */
export function rewriteEntityLinks(agentId: string, markdown: string): string {
    return markdown
        .split(/(```[\s\S]*?```|`[^`\n]*`)/g)
        .map((part, i) =>
            i % 2 === 1
                ? part
                : part.replace(LINK_RE, (match, target: string, alias?: string) => {
                      const parsed = parseEntityLink(alias === undefined ? target : `${target}|${alias}`)
                      if (!parsed) return match
                      const label = (parsed.alias ?? parsed.name).trim()
                      return `[${label}](${entityHref(agentId, parsed.kind, parsed.name)})`
                  })
        )
        .join('')
}

/** A `[[` token at the end of the input: the kind prefix once `:` is typed, else the query. */
export function entityToken(value: string): { start: number; kind: string | null; query: string } | null {
    const match = /(^|\s)\[\[([a-z]*)(?::([^\]\n|]*))?$/.exec(value)
    if (!match) return null
    const start = match.index + match[1].length
    if (match[3] !== undefined) return { start, kind: match[2] || null, query: match[3] }
    return { start, kind: null, query: match[2] ?? '' }
}

/** Replace the partial token with the complete link (a trailing space, like a mention). */
export function insertEntityLink(value: string, start: number, kind: EntityKind | 'memory', name: string): string {
    return `${value.slice(0, start)}[[${kind}:${name}]] `
}

/** Every entity link a text carries, in order, code spans excluded. */
export function collectEntityLinks(markdown: string): { kind: EntityKind; name: string }[] {
    const found: { kind: EntityKind; name: string }[] = []
    for (const part of markdown.split(/(```[\s\S]*?```|`[^`\n]*`)/g)) {
        if (!part || part.startsWith('```') || part.startsWith('`')) continue
        for (const match of part.matchAll(/\[\[([^\]\n|]+?)(?:\|[^\]\n]+)?\]\]/g)) {
            const parsed = parseEntityLink(match[1])
            if (parsed) found.push({ kind: parsed.kind, name: parsed.name })
        }
    }
    return found
}

/** The token a resolved link is keyed by, on the wire and in the broken set. */
export const linkKey = (kind: string, name: string) => `${kind}:${name}`

/**
 * The `kind:name` keys a scope has resolved as missing, provided by the surface that renders
 * the text (the conversation, a memory document). The Markdown layer turns a link in this set
 * into the broken state; null (no scope) renders every link optimistically live.
 */
export const EntityBroken = createContext<ReadonlySet<string> | null>(null)
