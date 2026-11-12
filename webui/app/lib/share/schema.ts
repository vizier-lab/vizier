// The snapshot format v1 (specs/056-shared-chat/contracts/snapshot-format.md), as the single
// definition the serializer, the preview, the viewer and the tests share. Every document is hostile
// input: unknown keys are dropped, unknown or malformed parts become placeholders, caps are enforced,
// and only a structural failure of the whole document is "damaged".
import { z } from 'zod'
import { magicMatches } from './sniff'
import { isAllowedMime, SHARE_LIMITS, SNAPSHOT_FORMAT_VERSION, type MediaMime } from './limits'

export type PlaceholderReason = 'oversized' | 'missing' | 'unsupported'

export interface TextPart { kind: 'text'; markdown: string }
export interface MediaPart { kind: 'media'; key: string; alt?: string }
export interface UiPart { kind: 'ui'; title: string; spec: Record<string, unknown>; state?: unknown }
export interface QuestionItem {
    id: string
    prompt: string
    type: string
    help?: string
    options?: { label: string; value: string; description?: string }[]
    answer?: { state: string; value?: string | string[] | boolean; label?: string }
}
export interface QuestionPart { kind: 'question'; title?: string; description?: string; state?: string; items: QuestionItem[] }
export interface PlaceholderPart { kind: 'placeholder'; reason: PlaceholderReason; label: string }
export type Part = TextPart | MediaPart | UiPart | QuestionPart | PlaceholderPart

export interface TrailEntry { kind: 'tool_call'; name: string; arguments: unknown; result?: string; ok?: boolean }
export interface Turn { id: string; role: 'person' | 'agent'; at: number; parts: Part[]; trail?: TrailEntry[] }
export interface MediaEntry { mime: MediaMime; bytes: number; data: string }
export interface Removals { credentials: number; paths: number; recall: number; reasoning: number; media: number }

export interface Snapshot {
    format_version: 1
    generator?: string
    created_at: number
    title: string
    source: { agent_name: string; agent_avatar?: string }
    include_trail: boolean
    turns: Turn[]
    media: Record<string, MediaEntry>
    rehydration: null
}

export type ParseResult =
    | { ok: true; snapshot: Snapshot; notes: string[] }
    | { ok: false; reason: 'needs_newer' | 'damaged'; message: string }

const KEY = /^[A-Za-z0-9_-]{1,64}$/
const BASE64 = /^[A-Za-z0-9+/]*={0,2}$/
const FORBIDDEN_KEYS = new Set(['__proto__', 'constructor', 'prototype'])
const enc = new TextEncoder()

/** UTF-8 length of a string. */
export function byteLength(s: string): number {
    return enc.encode(s).length
}

/** The nesting depth of a JSON-like value, counting stops at `limit + 1`. Iterative: a hostile document cannot overflow the stack. */
export function jsonDepth(value: unknown, limit: number = SHARE_LIMITS.jsonDepth): number {
    let max = 0
    const stack: [unknown, number][] = [[value, 1]]
    while (stack.length) {
        const [v, d] = stack.pop() as [unknown, number]
        if (v === null || typeof v !== 'object') continue
        if (d > max) max = d
        if (max > limit) return max
        for (const child of Array.isArray(v) ? v : Object.values(v)) stack.push([child, d + 1])
    }
    return max
}

/** A deep copy that drops `__proto__`-style keys, so a viewer that merges the value cannot be poisoned. Caller bounds the depth first. */
export function cleanJson<T>(value: T): T {
    if (Array.isArray(value)) return value.map(cleanJson) as T
    if (value && typeof value === 'object') {
        const out: Record<string, unknown> = {}
        for (const [k, v] of Object.entries(value)) if (!FORBIDDEN_KEYS.has(k)) out[k] = cleanJson(v)
        return out as T
    }
    return value
}

const text = (max: number) => z.string().refine((s) => byteLength(s) <= max)
const optionalText = (max: number) => z.string().max(max).optional()

const TextSchema = z.object({ kind: z.literal('text'), markdown: text(SHARE_LIMITS.textBytes) })
const MediaSchema = z.object({ kind: z.literal('media'), key: z.string().regex(KEY), alt: optionalText(2000) })
const UiSchema = z.object({
    kind: z.literal('ui'),
    title: z.string(),
    spec: z.record(z.string(), z.unknown()),
    state: z.unknown().optional(),
})
const AnswerSchema = z.object({
    state: z.string().max(40),
    value: z.union([z.string().max(20_000), z.array(z.string().max(2000)).max(100), z.boolean()]).optional().catch(undefined),
    label: optionalText(2000).catch(undefined),
})
const ItemSchema = z.object({
    id: z.string().max(200),
    prompt: z.string().max(10_000),
    type: z.string().max(40),
    help: optionalText(10_000).catch(undefined),
    options: z
        .array(z.object({ label: z.string().max(2000), value: z.string().max(2000), description: optionalText(2000).catch(undefined) }))
        .max(100)
        .optional()
        .catch(undefined),
    answer: AnswerSchema.optional().catch(undefined),
})
const QuestionSchema = z.object({
    kind: z.literal('question'),
    title: optionalText(1000).catch(undefined),
    description: optionalText(10_000).catch(undefined),
    state: optionalText(40).catch(undefined),
    items: z.array(ItemSchema).max(50),
})
const REASONS = ['oversized', 'missing', 'unsupported'] as const
const PlaceholderSchema = z.object({
    kind: z.literal('placeholder'),
    reason: z.enum(REASONS).catch('unsupported'),
    label: z.string().catch('').transform((s) => s.slice(0, 300)),
})

const placeholder = (reason: PlaceholderReason, label: string): PlaceholderPart => ({ kind: 'placeholder', reason, label })

/** One part, hostile: anything not understood becomes a placeholder, never rendered. */
function parsePart(raw: unknown, notes: string[]): Part {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return placeholder('unsupported', 'Content that this version cannot show')
    const kind = (raw as { kind?: unknown }).kind
    if (kind === 'text') {
        const r = TextSchema.safeParse(raw)
        if (r.success) return r.data
        const big = typeof (raw as { markdown?: unknown }).markdown === 'string'
        notes.push('text part rejected')
        return big ? placeholder('oversized', 'A message too large to show') : placeholder('unsupported', 'Content that this version cannot show')
    }
    if (kind === 'media') {
        const r = MediaSchema.safeParse(raw)
        return r.success && !FORBIDDEN_KEYS.has(r.data.key) ? (r.data.alt === undefined ? { kind: 'media', key: r.data.key } : r.data) : placeholder('unsupported', 'An image that cannot be shown')
    }
    if (kind === 'ui') {
        const r = UiSchema.safeParse(raw)
        if (!r.success) return placeholder('unsupported', 'An interactive page that cannot be shown')
        const title = r.data.title.slice(0, SHARE_LIMITS.titleLength)
        if (jsonDepth(r.data.spec) > SHARE_LIMITS.jsonDepth || jsonDepth(r.data.state) > SHARE_LIMITS.jsonDepth) {
            notes.push('interactive page too deeply nested')
            return placeholder('unsupported', title ? `Interactive page “${title}” (too complex to show)` : 'An interactive page too complex to show')
        }
        const spec = cleanJson(r.data.spec)
        if (byteLength(JSON.stringify(spec)) > SHARE_LIMITS.specBytes) {
            notes.push('interactive page too large')
            return placeholder('oversized', title ? `Interactive page “${title}” (too large to show)` : 'An interactive page too large to show')
        }
        const part: UiPart = { kind: 'ui', title, spec }
        if (r.data.state !== undefined) part.state = cleanJson(r.data.state)
        return part
    }
    if (kind === 'question') {
        const r = QuestionSchema.safeParse(raw)
        return r.success ? (r.data as QuestionPart) : placeholder('unsupported', 'A question that cannot be shown')
    }
    if (kind === 'placeholder') {
        const r = PlaceholderSchema.safeParse(raw)
        return r.success ? r.data : placeholder('unsupported', 'Content that this version cannot show')
    }
    return placeholder('unsupported', 'Content from a newer version of Vizier')
}

function parseTrail(raw: unknown): TrailEntry[] {
    if (!Array.isArray(raw)) return []
    const out: TrailEntry[] = []
    for (const e of raw) {
        if (!e || typeof e !== 'object' || (e as { kind?: unknown }).kind !== 'tool_call') continue
        const { name, arguments: args, result, ok } = e as Record<string, unknown>
        if (typeof name !== 'string') continue
        if (jsonDepth(args) > SHARE_LIMITS.jsonDepth) continue
        const entry: TrailEntry = { kind: 'tool_call', name: name.slice(0, 200), arguments: cleanJson(args ?? null) }
        if (typeof result === 'string' && byteLength(result) <= SHARE_LIMITS.textBytes) entry.result = result
        if (typeof ok === 'boolean') entry.ok = ok
        out.push(entry)
    }
    return out
}

const TopSchema = z.object({
    format_version: z.number().int().min(1),
    generator: z.string().max(200).optional().catch(undefined),
    created_at: z.number().finite().catch(0),
    title: z.string().catch(''),
    source: z.object({ agent_name: z.string().max(200).catch(''), agent_avatar: z.string().regex(KEY).optional().catch(undefined) }).catch({ agent_name: '' }),
    include_trail: z.boolean(),
    turns: z.array(z.unknown()).max(SHARE_LIMITS.turns),
    media: z.record(z.string(), z.unknown()).catch({}),
})

/** Bytes a base64 string decodes to. */
export function decodedLength(data: string): number {
    const pad = data.endsWith('==') ? 2 : data.endsWith('=') ? 1 : 0
    return Math.max(0, Math.floor((data.length * 3) / 4) - pad)
}

const damaged = (message: string): ParseResult => ({ ok: false, reason: 'damaged', message })

/** Reads a snapshot from text or an already parsed value. Never throws. */
export function parseSnapshot(input: unknown): ParseResult {
    let value: unknown = input
    if (typeof input === 'string') {
        try {
            value = JSON.parse(input)
        } catch {
            return damaged('The file is not valid JSON.')
        }
    }
    if (!value || typeof value !== 'object' || Array.isArray(value)) return damaged('The document is not an object.')
    const version = (value as { format_version?: unknown }).format_version
    if (typeof version === 'number' && Number.isInteger(version) && version > SNAPSHOT_FORMAT_VERSION) {
        return { ok: false, reason: 'needs_newer', message: `This conversation was saved in format ${version}; open it with a newer version of Vizier.` }
    }
    if (version !== SNAPSHOT_FORMAT_VERSION) return damaged('The format version is missing or not recognised.')
    const top = TopSchema.safeParse(value)
    if (!top.success) return damaged('The document does not match the snapshot format.')
    const t = top.data
    const notes: string[] = []

    // Media: raster allow-list, capped, base64 only. A refused entry is remembered by why, for its references.
    const media: Record<string, MediaEntry> = {}
    const refused = new Map<string, PlaceholderReason>()
    const keys = Object.keys(t.media)
    if (keys.length > SHARE_LIMITS.mediaItems * 4) return damaged('The document holds too many images.')
    let accepted = 0
    for (const key of keys) {
        const raw = t.media[key] as { mime?: unknown; bytes?: unknown; data?: unknown } | null
        if (!KEY.test(key) || FORBIDDEN_KEYS.has(key)) continue
        if (!raw || typeof raw !== 'object' || typeof raw.mime !== 'string' || typeof raw.data !== 'string') {
            refused.set(key, 'unsupported')
            continue
        }
        if (!isAllowedMime(raw.mime)) {
            refused.set(key, 'unsupported')
            notes.push(`media ${key} refused: ${raw.mime.slice(0, 40)}`)
            continue
        }
        // 4 base64 characters carry 3 bytes.
        if (raw.data.length > Math.ceil((SHARE_LIMITS.mediaBytes * 4) / 3) + 8) {
            refused.set(key, 'oversized')
            continue
        }
        if (!BASE64.test(raw.data)) {
            refused.set(key, 'unsupported')
            continue
        }
        // The claimed type must be the real one: a `image/png` whose bytes are SVG or HTML is not an image.
        if (!magicMatches(raw.mime, raw.data)) {
            refused.set(key, 'unsupported')
            notes.push(`media ${key} refused: bytes are not ${raw.mime}`)
            continue
        }
        if (accepted >= SHARE_LIMITS.mediaItems) {
            refused.set(key, 'oversized')
            continue
        }
        accepted++
        media[key] = { mime: raw.mime, bytes: decodedLength(raw.data), data: raw.data }
    }

    const referenced = new Set<string>()
    const turns: Turn[] = []
    for (const [i, rawTurn] of t.turns.entries()) {
        if (!rawTurn || typeof rawTurn !== 'object') return damaged(`Turn ${i + 1} is not an object.`)
        const rt = rawTurn as Record<string, unknown>
        if ((rt.role !== 'person' && rt.role !== 'agent') || !Array.isArray(rt.parts)) return damaged(`Turn ${i + 1} is malformed.`)
        if (rt.parts.length > SHARE_LIMITS.parts) return damaged(`Turn ${i + 1} has too many parts.`)
        const parts = rt.parts.map((p) => parsePart(p, notes)).map((p): Part => {
            if (p.kind !== 'media') return p
            if (media[p.key]) {
                referenced.add(p.key)
                return p
            }
            return placeholder(refused.get(p.key) ?? 'missing', refused.get(p.key) === 'oversized' ? 'An image too large to include' : refused.get(p.key) === 'unsupported' ? 'An image of a kind that is not shown' : 'An image that is no longer available')
        })
        const turn: Turn = {
            id: typeof rt.id === 'string' ? rt.id.slice(0, 64) : `t${i + 1}`,
            role: rt.role,
            at: typeof rt.at === 'number' && Number.isFinite(rt.at) ? rt.at : 0,
            parts,
        }
        if (rt.trail !== undefined) {
            if (!t.include_trail) return damaged('The document has activity trail entries but says it has none.')
            turn.trail = parseTrail(rt.trail)
        }
        turns.push(turn)
    }

    // An unreferenced key is dropped, as the contract says.
    for (const key of Object.keys(media)) if (!referenced.has(key) && !referencedBySpec(turns, key) && key !== t.source.agent_avatar) delete media[key]

    const snapshot: Snapshot = {
        format_version: 1,
        created_at: t.created_at,
        title: t.title.slice(0, SHARE_LIMITS.titleLength),
        source: t.source.agent_avatar && media[t.source.agent_avatar] ? { agent_name: t.source.agent_name, agent_avatar: t.source.agent_avatar } : { agent_name: t.source.agent_name },
        include_trail: t.include_trail,
        turns,
        media,
        rehydration: null,
    }
    if (t.generator) snapshot.generator = t.generator
    return { ok: true, snapshot, notes }
}

/** Whether any interactive page names this media key as a string anywhere in its spec or state. */
function referencedBySpec(turns: Turn[], key: string): boolean {
    for (const turn of turns) {
        for (const part of turn.parts) {
            if (part.kind !== 'ui') continue
            const stack: unknown[] = [part.spec, part.state]
            while (stack.length) {
                const v = stack.pop()
                if (v === key) return true
                if (v && typeof v === 'object') stack.push(...(Array.isArray(v) ? v : Object.values(v)))
            }
        }
    }
    return false
}
