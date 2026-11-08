// Saved history -> snapshot v1 (contracts/snapshot-format.md). Everything that must not leave is
// decided here: the kinds table (kinds.ts) says what is in, redact.ts and paths.ts clean what is kept,
// and every removal is counted so the preview and the stored `removals` agree.
import { topicFiles, topicHistory } from '../../api/endpoints/chat'
import type { ChatMessage, SessionFileRecord, VizierAttachment } from '../../api/frames'
import { included, entryDisposition, UI_RENDER } from './kinds'
import { SHARE_LIMITS, SNAPSHOT_FORMAT_VERSION } from './limits'
import { MediaBuilder, rewriteSpecFiles, specFileNames, type MediaDeps, defaultMediaDeps } from './media'
import { stripPaths } from './paths'
import { newCounts, redactText, redactValue } from './redact'
import { byteLength, parseSnapshot, type Part, type PlaceholderPart, type PlaceholderReason, type Removals, type Snapshot, type TrailEntry, type Turn } from './schema'

export type ShareErrorCode = 'too_many_turns' | 'empty' | 'too_large' | 'invalid' | 'people' | 'audience'

export class ShareError extends Error {
    constructor(
        readonly code: ShareErrorCode,
        message: string
    ) {
        super(message)
        this.name = 'ShareError'
    }
}

export interface SerializeOptions {
    agentName: string
    /** The conversation's title; empty falls back to a plain default. */
    title?: string
    /** Activity trail on or off. Default off (FR-009). */
    includeTrail?: boolean
    /** Files of the conversation, for images and attachments. */
    files?: readonly SessionFileRecord[]
    /** True while a turn is streaming; undefined = decide from the shape of the history. */
    turnInProgress?: boolean
    now?: number
    generator?: string
    media?: MediaDeps
}

export interface SerializeResult {
    snapshot: Snapshot
    removals: Removals
    /** Images and files that became placeholders, once each. */
    placeholders: { name: string; reason: PlaceholderReason; label: string }[]
    /** Trail entries (tool calls, results) left out because the trail is off. */
    omittedTrail: number
    /** The last turn was still running and is not in the snapshot. */
    turnInProgress: boolean
    /** Text parts cut at the per-part limit. */
    truncated: number
}

export const DEFAULT_TITLE = 'Shared conversation'

/** Removes credentials, then directory parts, from one string. */
export function cleanText(text: string, counts: Removals): string {
    return stripPaths(redactText(text, counts), counts)
}

function cleanStrings(value: unknown, counts: Removals, depth = 0): unknown {
    if (typeof value === 'string') return stripPaths(value, counts)
    if (value === null || typeof value !== 'object' || depth > 256) return value
    if (Array.isArray(value)) return value.map((v) => cleanStrings(v, counts, depth + 1))
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, cleanStrings(v, counts, depth + 1)]))
}

/** Credentials and paths removed from every string of a JSON value. */
export function cleanValue(value: unknown, counts: Removals): unknown {
    return cleanStrings(redactValue(value, counts), counts)
}

function cutBytes(s: string, max: number): { text: string; cut: boolean } {
    if (byteLength(s) <= max) return { text: s, cut: false }
    let end = Math.min(s.length, max)
    while (byteLength(s.slice(0, end)) > max - 32) end = Math.floor(end * 0.9)
    // do not split a surrogate pair
    if (end > 0 && /[\ud800-\udbff]/.test(s[end - 1])) end--
    return { text: `${s.slice(0, end)}\n\n… [cut: too long to include]`, cut: true }
}

const ms = (iso: unknown): number => {
    const t = typeof iso === 'string' ? Date.parse(iso) : NaN
    return Number.isFinite(t) ? t : 0
}

/** Entries that end a turn: a reply, a question card, a page, a command. */
function endsTurn(e: ChatMessage): boolean {
    const c = e.content
    if (c.Response) return typeof c.Response.content === 'object' ? ['message', 'audio_reply', 'error'].some((k) => k in (c.Response?.content as object)) : c.Response.content === 'empty' || c.Response.content === 'abort'
    if (c.QuestionSet || c.Command || c.Checkpoint !== undefined) return true
    if (c.ClientToolCall) return c.ClientToolCall.name === UI_RENDER
    return false
}

/** Index to cut the entries at so an unfinished last turn is left out (or `entries.length`). */
export function completedLength(entries: readonly ChatMessage[], force?: boolean): number {
    if (force === false) return entries.length
    let lastEnd = -1
    let lastRequest = -1
    entries.forEach((e, i) => {
        if (endsTurn(e)) lastEnd = i
        if (e.content.Request && entryDisposition(e.content as Record<string, unknown>).disposition !== 'never') lastRequest = i
    })
    const tail = entries.length - 1
    const unfinished = force === true ? true : tail >= 0 && lastEnd < tail && entries.slice(lastEnd + 1).some((e) => !e.content.Recall && !e.content.SensitiveAsk)
    if (!unfinished) return entries.length
    return lastRequest > lastEnd ? lastRequest : lastEnd + 1
}

/** Builds the snapshot from entries already in stored order. */
export async function serializeEntries(allEntries: readonly ChatMessage[], opts: SerializeOptions): Promise<SerializeResult> {
    const includeTrail = opts.includeTrail === true
    const counts = newCounts()
    const media = new MediaBuilder(opts.files ?? [], opts.media ?? defaultMediaDeps)
    const cut = completedLength(allEntries, opts.turnInProgress)
    const entries = allEntries.slice(0, cut)
    const turnInProgress = cut < allEntries.length || opts.turnInProgress === true
    let omittedTrail = 0
    let truncated = 0

    const turns: Turn[] = []
    let agent: Turn | null = null
    const pending = new Map<string, TrailEntry>()

    const newTurn = (role: Turn['role'], at: number): Turn => ({ id: '', role, at, parts: [] })
    const close = () => {
        if (agent && (agent.parts.length || agent.trail?.length)) turns.push(agent)
        agent = null
        pending.clear()
    }
    const current = (at: number): Turn => {
        if (agent && agent.parts.length >= SHARE_LIMITS.parts) close()
        return (agent ??= newTurn('agent', at))
    }
    const text = (turn: Turn, raw: string) => {
        if (!raw.trim()) return
        const { text: cleaned, cut: wasCut } = cutBytes(cleanText(raw, counts), SHARE_LIMITS.textBytes)
        if (wasCut) truncated++
        turn.parts.push({ kind: 'text', markdown: cleaned })
    }
    const placeholder = (turn: Turn, reason: PlaceholderReason, label: string) => turn.parts.push({ kind: 'placeholder', reason, label: cleanText(label, counts) } satisfies PlaceholderPart)
    const attachments = async (turn: Turn, list: VizierAttachment[] | undefined) => {
        for (const a of list ?? []) {
            const inline = 'bytes' in a.content ? Uint8Array.from(a.content.bytes) : 'base64' in a.content ? Uint8Array.from(atob(a.content.base64), (ch) => ch.charCodeAt(0)) : undefined
            const out = await media.add(a.filename, inline)
            if (out.ok) turn.parts.push({ kind: 'media', key: out.key, alt: cleanText(a.filename.split(/[\\/]/).pop() ?? '', counts) })
            else placeholder(turn, out.reason, out.label)
        }
    }

    for (const entry of entries) {
        const content = entry.content as Record<string, unknown>
        const { kind, disposition } = entryDisposition(content)
        const at = ms(entry.timestamp)

        // Counted removals first: they happen whether or not the trail is on.
        if (kind === 'Recall') counts.recall++
        if (kind === 'SensitiveAsk') counts.credentials++
        if (kind === 'Response' && typeof entry.content.Response?.content === 'object' && entry.content.Response.content && 'thinking' in entry.content.Response.content) counts.reasoning++

        if (!included(disposition, includeTrail)) {
            if (disposition === 'trail-on') omittedTrail++
            continue
        }

        if (kind === 'Request') {
            const req = entry.content.Request!
            close()
            const turn = newTurn('person', ms(req.timestamp) || at)
            const c = req.content as unknown as Record<string, unknown>
            if ('chat' in c) text(turn, String(c.chat))
            else if ('client_event' in c) text(turn, String((c.client_event as { text?: string }).text ?? ''))
            else if ('question_answer' in c) text(turn, String((c.question_answer as { text?: string }).text ?? 'Answered a question'))
            else if ('audio_chat' in c) {
                const [, transcript] = c.audio_chat as [unknown, string | null]
                if (transcript) text(turn, transcript)
                placeholder(turn, 'unsupported', 'Voice message (audio is not included)')
            }
            await attachments(turn, req.attachments)
            if (turn.parts.length) turns.push(turn)
            continue
        }

        const turn = current(at)
        if (kind === 'Response') {
            const r = entry.content.Response!
            const body = r.content
            if (typeof body === 'object' && body && 'message' in body) {
                text(turn, body.message.content)
                await attachments(turn, r.attachments)
            } else if (typeof body === 'object' && body && ('tool_choice' in body || 'tool_response' in body)) {
                trailAdd(turn, 'tool_choice' in body ? { kind: 'tool_call', name: body.tool_choice.name, arguments: cleanValue(body.tool_choice.args, counts) } : { kind: 'tool_call', name: 'tool_response', arguments: null, result: resultText(body.tool_response.response, counts) })
            }
        } else if (kind === 'AssistantMessage') {
            text(turn, entry.content.AssistantMessage ?? '')
        } else if (kind === 'QuestionSet') {
            turn.parts.push(questionPart(entry.content.QuestionSet!, counts))
        } else if (kind === 'ClientToolCall') {
            const call = entry.content.ClientToolCall!
            if (call.name === UI_RENDER) await uiPart(turn, call, media, counts)
            else {
                const outcome = call.outcome
                trailAdd(turn, {
                    kind: 'tool_call',
                    name: call.name,
                    arguments: argsOf(cleanValue(call.arguments, counts)),
                    ...(outcome && outcome.status === 'ok' ? { result: resultText(outcome.result, counts), ok: true } : outcome && outcome.status !== 'pending' ? { ok: false, result: outcome.status === 'error' ? cleanText(outcome.message, counts).slice(0, 2000) : outcome.status } : {}),
                })
            }
        } else if (kind === 'ToolCall') {
            const call = entry.content.ToolCall!
            const t: TrailEntry = { kind: 'tool_call', name: call.name, arguments: argsOf(cleanValue(call.arguments, counts)) }
            pending.set(call.call_id, t)
            trailAdd(turn, t)
        } else if (kind === 'ToolResult') {
            const res = entry.content.ToolResult!
            const t = pending.get(res.call_id)
            const body = cutResult(cleanText(res.content, counts))
            if (t) t.result = body
            else trailAdd(turn, { kind: 'tool_call', name: 'result', arguments: null, result: body })
        }
    }
    close()

    function trailAdd(turn: Turn, t: TrailEntry) {
        ;(turn.trail ??= []).push(t)
    }
    const placeholders = media.left
    counts.media = placeholders.length

    // Ids in order, and the 5,000 turn cap as a refusal rather than a silent cut.
    if (turns.length > SHARE_LIMITS.turns) throw new ShareError('too_many_turns', tooManyTurns())
    turns.forEach((t, i) => (t.id = `t${i + 1}`))

    const title = cleanText((opts.title ?? '').trim(), counts).slice(0, SHARE_LIMITS.titleLength)
    const snapshot: Snapshot = {
        format_version: SNAPSHOT_FORMAT_VERSION,
        generator: opts.generator ?? 'vizier-webui',
        created_at: opts.now ?? Date.now(),
        title: title || DEFAULT_TITLE,
        source: { agent_name: cleanText(opts.agentName, counts).slice(0, 200) },
        include_trail: includeTrail,
        turns,
        media: media.media,
        rehydration: null,
    }
    // Nothing the viewer would refuse may be offered.
    const check = parseSnapshot(snapshot)
    if (!check.ok) throw new ShareError('invalid', check.message)
    return { snapshot, removals: counts, placeholders, omittedTrail, turnInProgress, truncated }
}

export const tooManyTurns = () => `This conversation has more than ${SHARE_LIMITS.turns.toLocaleString('en')} turns, which is more than a shared conversation can hold. Start a new conversation with the part you want to share.`

function cutResult(s: string): string {
    return s.length > SHARE_LIMITS.trailResultChars ? `${s.slice(0, SHARE_LIMITS.trailResultChars)}\n… [cut: ${s.length - SHARE_LIMITS.trailResultChars} more characters]` : s
}

function resultText(value: unknown, counts: Removals): string {
    const s = typeof value === 'string' ? value : (JSON.stringify(value) ?? '')
    return cutResult(cleanText(s, counts))
}

function argsOf(value: unknown): unknown {
    const size = byteLength(JSON.stringify(value) ?? 'null')
    return size > SHARE_LIMITS.trailArgumentBytes ? { note: `arguments cut: ${size} bytes` } : value
}

function questionPart(q: NonNullable<ChatMessage['content']['QuestionSet']>, counts: Removals): Part {
    const answers = new Map((q.answers ?? []).map((a) => [a.question_id, a]))
    // `answerer` is a person's name: it is never copied.
    const part: Part = {
        kind: 'question',
        ...(q.title ? { title: cleanText(q.title, counts) } : {}),
        ...(q.description ? { description: cleanText(q.description, counts) } : {}),
        state: q.state,
        items: q.questions.map((item) => {
            const a = answers.get(item.id)
            return {
                id: item.id,
                prompt: cleanText(item.prompt, counts),
                type: item.type,
                ...(item.help ? { help: cleanText(item.help, counts) } : {}),
                ...(item.options?.length ? { options: item.options.map((o) => ({ label: cleanText(o.label, counts), value: cleanText(o.value, counts), ...(o.description ? { description: cleanText(o.description, counts) } : {}) })) } : {}),
                ...(a ? { answer: { state: a.state, ...(a.value !== undefined && a.value !== null ? { value: cleanAnswer(a.value, counts) } : {}), ...(a.label ? { label: cleanText(a.label, counts) } : {}) } } : {}),
            }
        }),
    }
    return part
}

function cleanAnswer(v: string | string[] | boolean, counts: Removals): string | string[] | boolean {
    return typeof v === 'string' ? cleanText(v, counts) : Array.isArray(v) ? v.map((s) => cleanText(s, counts)) : v
}

async function uiPart(turn: Turn, call: NonNullable<ChatMessage['content']['ClientToolCall']>, media: MediaBuilder, counts: Removals) {
    const args = call.arguments as { title?: unknown; spec?: unknown }
    const title = typeof args?.title === 'string' ? cleanText(args.title, counts).slice(0, SHARE_LIMITS.titleLength) : ''
    if (!args?.spec || typeof args.spec !== 'object' || Array.isArray(args.spec)) {
        turn.parts.push({ kind: 'placeholder', reason: 'unsupported', label: title ? `Interactive page “${title}” cannot be shown` : 'An interactive page that cannot be shown' })
        return
    }
    const keys = new Map<string, string>()
    for (const name of specFileNames(args.spec)) keys.set(name, await media.keyFor(name))
    const spec = cleanValue(rewriteSpecFiles(args.spec, keys), counts) as Record<string, unknown>
    const saved = call.state !== undefined && call.state !== null ? call.state : spec.state
    const part: Part = { kind: 'ui', title, spec }
    if (saved !== undefined) part.state = cleanValue(saved, counts)
    turn.parts.push(part)
}

// -- reading the saved history -------------------------------------------------------------------

export interface HistoryReader {
    page(cursor: { before?: string; before_seq?: number | null; limit: number }): Promise<ChatMessage[]>
}

export const HISTORY_PAGE = 200

/** Every saved entry, oldest first: pages are read backwards until one comes back short or repeats. */
export async function readAllHistory(reader: HistoryReader, opts: { pageSize?: number; maxRequests?: number } = {}): Promise<ChatMessage[]> {
    const size = opts.pageSize ?? HISTORY_PAGE
    const maxRequests = opts.maxRequests ?? SHARE_LIMITS.turns
    const seen = new Set<string>()
    let all: ChatMessage[] = []
    let cursor: { before?: string; before_seq?: number | null } = {}
    for (let guard = 0; guard < 10_000; guard++) {
        const page = await reader.page({ ...cursor, limit: size })
        const fresh = page.filter((e) => !seen.has(e.uid))
        if (fresh.length === 0) break
        fresh.forEach((e) => seen.add(e.uid))
        all = [...fresh, ...all]
        if (all.filter((e) => e.content.Request).length > maxRequests) throw new ShareError('too_many_turns', tooManyTurns())
        if (page.length < size) break
        const oldest = page[0]
        cursor = { before: oldest.uid, before_seq: oldest.seq ?? null }
    }
    return all
}

export const apiHistoryReader = (agentId: string, topicId: string): HistoryReader => ({
    page: (cursor) => topicHistory(agentId, topicId, cursor),
})

/** Every file record of the conversation, all pages. */
export async function readAllFiles(agentId: string, topicId: string): Promise<SessionFileRecord[]> {
    const out: SessionFileRecord[] = []
    for (let offset = 0; offset < 10_000; offset += 100) {
        const page = await topicFiles(agentId, topicId, offset, 100)
        out.push(...page.items)
        if (page.items.length < 100 || out.length >= page.total) break
    }
    return out
}

export interface ConversationInput extends Omit<SerializeOptions, 'files'> {
    agentId: string
    topicId: string
    /** For tests and previews built from data already in hand. */
    reader?: HistoryReader
    files?: readonly SessionFileRecord[]
}

/** Reads the whole saved conversation and its files and builds the snapshot. */
export async function serializeConversation(input: ConversationInput): Promise<SerializeResult> {
    const entries = await readAllHistory(input.reader ?? apiHistoryReader(input.agentId, input.topicId))
    const files = input.files ?? (await readAllFiles(input.agentId, input.topicId))
    return serializeEntries(entries, { ...input, files })
}
