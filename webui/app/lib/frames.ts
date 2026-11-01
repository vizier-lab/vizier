// The pure turn reducer: `(TopicState, Action) → TopicState` (data-model §3). Everything a
// topic's page needs comes out of it, and nothing in it touches the network or the clock, so
// it is unit-tested without a browser.
//
// The running trail is built from `history_entry` frames merged by uid (spec 023), never from
// `thinking`/`tool_choice`: those only set the live label. That is what lets a page reopened
// mid-turn, or a second tab, draw the same trail as the tab that sent the message.

import type {
    BackgroundJobSnapshot,
    ChatMessage,
    Frame,
    QuestionRefusal,
    QuestionSetUpdate,
    SensitiveAsk,
    SessionFileRecord,
    VizierAttachment,
    VizierResponseContent,
    WebSocketResponse,
} from '../api/frames'
import { applyClientOutcome, applyClientState, appendClientCall, mergeEntries } from './trail'

export type Phase = 'idle' | 'thinking' | 'working'

export interface QueuedMessage {
    id: string
    text: string
    attachments?: VizierAttachment[]
    spoken?: boolean
}

export interface TopicError {
    kind: string
    message: string
}

export interface TopicState {
    /** History entries in stored order, merged by uid. Synthetic ones have a `live-` uid. */
    entries: ChatMessage[]
    phase: Phase
    /** "Archivist is running execute_python…" — from `tool_choice` only. */
    label: string | null
    /** Sent while a turn was running; held here and sent when it ends. */
    queued: QueuedMessage[]
    jobs: Record<string, BackgroundJobSnapshot>
    /** Newest first. */
    files: SessionFileRecord[]
    /** From the latest reply's stats: feeds the context ring. */
    context: { used: number; window: number } | null
    error: TopicError | null
    /** The latest reply carried on after a restart. */
    resumed: boolean
    /** The person stopped the turn that was running. */
    stopped: boolean
    /** The generated title, once the server sends it; the topic list's copy is older. */
    title: string | null
    session: ChatMessage['vizier_session']
    /** Sensitive-input asks open on this conversation, by ask id. The dialog shows the oldest. */
    asks: Record<string, SensitiveAsk>
    /** The latest refusal of an answer to each question set, so its card names the question. */
    refusals: Record<string, QuestionRefusal>
}

export type Action =
    | { type: 'frame'; frame: Frame; now?: string }
    | { type: 'history'; entries: ChatMessage[]; position?: 'latest' | 'older' }
    | { type: 'busy'; thinking: boolean }
    | { type: 'send'; id: string; text: string; attachments?: VizierAttachment[]; spoken?: boolean; now?: string }
    | { type: 'dequeue'; now?: string }
    | { type: 'abort'; now?: string }
    | { type: 'files'; files: SessionFileRecord[] }
    | { type: 'jobs'; jobs: BackgroundJobSnapshot[] }

export const isLive = (entry: ChatMessage) => entry.uid.startsWith('live-')

export function emptyTopic(session: ChatMessage['vizier_session'] = { agent_id: '', channel: '', topic: '' }): TopicState {
    return {
        entries: [],
        phase: 'idle',
        label: null,
        queued: [],
        jobs: {},
        files: [],
        context: null,
        error: null,
        resumed: false,
        stopped: false,
        title: null,
        session,
        asks: {},
        refusals: {},
    }
}

const live = (state: TopicState, uid: string, timestamp: string, content: ChatMessage['content']): ChatMessage => ({
    uid: `live-${uid}`,
    timestamp,
    vizier_session: state.session,
    content,
    serverUid: false,
})

function requestEntry(state: TopicState, id: string, text: string, now: string, attachments?: VizierAttachment[]): ChatMessage {
    return live(state, `req-${id}`, now, {
        Request: { timestamp: now, user: '', content: { chat: text }, ...(attachments?.length ? { attachments } : {}) },
    })
}

/** The `client_error` codes that refuse an answer to a question set. */
const QUESTION_ERRORS = new Set(['question_not_open', 'invalid_answer', 'answer_too_large', 'rate_limited', 'question_not_found', 'question_not_recorded'])

function withoutKey<T>(record: Record<string, T>, key: string): Record<string, T> {
    if (!(key in record)) return record
    const { [key]: _gone, ...rest } = record
    return rest
}

/** A set's state, answers and answerer, as the server last said. Nothing else about the entry changes. */
function applyQuestionUpdate(entries: ChatMessage[], update: QuestionSetUpdate): ChatMessage[] {
    return entries.map((entry) => {
        const set = entry.content.QuestionSet
        if (!set || set.set_id !== update.set_id) return entry
        return {
            ...entry,
            content: {
                ...entry.content,
                QuestionSet: { ...set, state: update.state, answers: update.answers ?? null, answerer: update.answerer ?? null },
            },
        }
    })
}

/** The turn ended: nothing is running and the label goes. */
const idle = (state: TopicState): TopicState => ({ ...state, phase: 'idle', label: null })

function replyEntry(state: TopicState, response: WebSocketResponse): ChatMessage {
    const uid = response.history_uid ?? `reply-${response.timestamp}`
    return {
        uid: response.history_uid ?? `live-${uid}`,
        timestamp: response.timestamp,
        vizier_session: state.session,
        content: { Response: { timestamp: response.timestamp, content: response.content, attachments: response.attachments, resumed: response.resumed } },
        serverUid: response.history_uid !== undefined,
    }
}

function statsOf(content: VizierResponseContent) {
    if (typeof content !== 'object' || content === null) return undefined
    if ('message' in content) return content.message.stats
    if ('audio_reply' in content) return content.audio_reply[2] ?? undefined
    return undefined
}

/** The ring's reading from the newest stored reply that has one, so a reopened topic shows it before the next reply. */
function contextOfHistory(entries: ChatMessage[]): TopicState['context'] {
    for (let i = entries.length - 1; i >= 0; i--) {
        const content = entries[i].content
        const reply = typeof content === 'object' && content !== null && 'Response' in content ? content.Response : undefined
        if (!reply) continue
        const stats = statsOf(reply.content)
        if (stats?.current_context_size && stats.context_window) return { used: stats.current_context_size, window: stats.context_window }
    }
    return null
}

function onResponse(state: TopicState, response: WebSocketResponse, now: string): TopicState {
    const { content } = response

    if (content === 'thinking_start') {
        return { ...state, phase: 'thinking', label: null, error: null, stopped: false, resumed: false }
    }
    if (content === 'empty') return state
    if (content === 'abort') {
        return { ...idle(state), queued: [], stopped: true }
    }
    if (typeof content !== 'object' || content === null) return state

    if ('thinking' in content) return { ...state, phase: 'working' }
    if ('tool_choice' in content) return { ...state, phase: 'working', label: content.tool_choice.name }
    if ('tool_response' in content) return { ...state, phase: 'working', label: null }

    if ('checkpoint' in content) {
        const entry = live(state, `checkpoint-${response.timestamp}`, response.timestamp, {
            Checkpoint: content.checkpoint.handover ?? undefined,
        })
        return { ...state, entries: mergeEntries(state.entries, [entry]) }
    }

    if ('message' in content || 'audio_reply' in content) {
        const stats = statsOf(content)
        const entry = replyEntry(state, response)
        const context =
            stats?.current_context_size && stats.context_window
                ? { used: stats.current_context_size, window: stats.context_window }
                : state.context
        return {
            ...idle(state),
            entries: mergeEntries(state.entries, [entry]),
            context,
            resumed: response.resumed === true,
            error: null,
        }
    }

    if ('error' in content) {
        const entry = replyEntry(state, response)
        return {
            ...idle(state),
            entries: mergeEntries(state.entries, [entry]),
            error: { kind: content.error.kind, message: content.error.message },
        }
    }
    void now
    return state
}

export function reduce(state: TopicState, action: Action): TopicState {
    switch (action.type) {
        case 'frame': {
            const { frame } = action
            const now = action.now ?? new Date(0).toISOString()
            switch (frame.kind) {
                case 'response':
                    return onResponse(state, frame.response, now)
                case 'history_entry': {
                    // A step of a turn nobody here started (a woken report, another tab) makes
                    // the topic busy.
                    const busy = state.phase === 'idle' && !frame.entry.content.Response && !frame.entry.content.Checkpoint && !frame.entry.content.Command
                    // A stored request replaces the stand-in drawn for it when it was sent, in the
                    // stand-in's place: appended, it would land after the steps already shown.
                    const standIn = frame.entry.content.Request
                        ? state.entries.findIndex((e) => isLive(e) && e.content.Request && coveredByStored(e, [frame.entry]))
                        : -1
                    const base =
                        standIn >= 0 && !state.entries.some((e) => e.uid === frame.entry.uid)
                            ? state.entries.map((e, i) => (i === standIn ? frame.entry : e))
                            : frame.entry.content.Request
                              ? state.entries.filter((e) => !(isLive(e) && e.content.Request && coveredByStored(e, [frame.entry])))
                              : state.entries
                    return {
                        ...state,
                        phase: busy ? 'working' : state.phase,
                        entries: mergeEntries(base, [frame.entry]),
                    }
                }
                case 'job':
                    return { ...state, jobs: { ...state.jobs, [frame.snapshot.id]: frame.snapshot } }
                case 'reactions':
                    return {
                        ...state,
                        entries: state.entries.map((e) =>
                            e.uid === frame.messageUid ? { ...e, reactions: frame.reactions } : e
                        ),
                    }
                case 'reaction_error':
                    return state
                case 'title':
                    return { ...state, title: frame.title }
                case 'session_file': {
                    const rest = state.files.filter((f) => f.id !== frame.file.id)
                    return { ...state, files: [frame.file, ...rest] }
                }
                case 'question_set':
                    return {
                        ...state,
                        entries: applyQuestionUpdate(state.entries, frame.update),
                        // A set that closed has nothing left to refuse.
                        refusals: withoutKey(state.refusals, frame.update.set_id),
                    }
                case 'sensitive_ask':
                    return { ...state, asks: { ...state.asks, [frame.ask.ask_id]: frame.ask } }
                case 'sensitive_ask_closed':
                    return { ...state, asks: withoutKey(state.asks, frame.askId) }
                case 'client': {
                    const f = frame.frame
                    // The ask is gone on the server (answered elsewhere, timed out): so is its dialog.
                    if ('client_error' in f && f.client_error.code === 'sensitive_ask_not_open' && f.client_error.call_id) {
                        return { ...state, asks: withoutKey(state.asks, f.client_error.call_id) }
                    }
                    if ('client_error' in f && QUESTION_ERRORS.has(f.client_error.code) && f.client_error.set_id) {
                        return {
                            ...state,
                            refusals: {
                                ...state.refusals,
                                [f.client_error.set_id]: {
                                    code: f.client_error.code,
                                    message: f.client_error.message,
                                    questionId: f.client_error.question_id,
                                },
                            },
                        }
                    }
                    if ('client_tool_call' in f) {
                        return { ...state, entries: appendClientCall(state.entries, f.client_tool_call, state.session) }
                    }
                    if ('client_tool_outcome' in f) {
                        return { ...state, entries: applyClientOutcome(state.entries, f.client_tool_outcome) }
                    }
                    if ('client_call_state' in f) {
                        return { ...state, entries: applyClientState(state.entries, f.client_call_state) }
                    }
                    return state
                }
            }
            return state
        }

        case 'history': {
            // What the server stored replaces the stand-ins drawn while the turn ran, and
            // keeps its own order: a page of older entries goes above what is held, the
            // newest page replaces its own span of it, in place.
            const stored = action.entries
            if (action.position === 'older') {
                const held = state.entries.filter((e) => !isLive(e))
                const have = new Set(held.map((e) => e.uid))
                const keptLive = state.entries.filter(
                    (e) => isLive(e) && !stored.some((s) => s.uid === e.uid) && !coveredByStored(e, stored)
                )
                const merged = [...stored.filter((e) => !have.has(e.uid)), ...held]
                return { ...state, entries: keptLive.length ? [...merged, ...keptLive] : merged }
            }
            return {
                ...state,
                context: state.context ?? contextOfHistory(stored),
                entries: mergePage(state.entries, stored),
            }
        }

        case 'busy':
            return action.thinking
                ? { ...state, phase: state.phase === 'idle' ? 'working' : state.phase }
                : idle(state)

        case 'send': {
            const now = action.now ?? new Date(0).toISOString()
            if (state.phase !== 'idle') {
                return { ...state, queued: [...state.queued, { id: action.id, text: action.text, attachments: action.attachments, spoken: action.spoken }] }
            }
            return {
                ...state,
                phase: 'thinking',
                error: null,
                stopped: false,
                entries: mergeEntries(state.entries, [requestEntry(state, action.id, action.text, now, action.attachments)]),
            }
        }

        case 'dequeue': {
            const [next, ...rest] = state.queued
            if (!next || state.phase !== 'idle') return state
            const now = action.now ?? new Date(0).toISOString()
            return {
                ...state,
                queued: rest,
                phase: 'thinking',
                error: null,
                stopped: false,
                entries: mergeEntries(state.entries, [requestEntry(state, next.id, next.text, now, next.attachments)]),
            }
        }

        case 'abort': {
            // The server records the abort as a `Command` entry, and `groupHistory` ends the
            // turn at it: that is where the turn's trail renders. Drawn here the moment it is
            // pressed, so a message sent straight after lands below the trail it ended rather
            // than above it (a reload used to be what put it right).
            const now = action.now ?? new Date(0).toISOString()
            const entry = live(state, `abort-${now}`, now, { Command: 'abort' })
            return { ...idle(state), queued: [], stopped: true, entries: mergeEntries(state.entries, [entry]) }
        }

        case 'files':
            return { ...state, files: action.files }

        case 'jobs':
            return { ...state, jobs: Object.fromEntries(action.jobs.map((j) => [j.id, j])) }
    }
}

/** How far apart a stand-in request and its stored entry can be stamped: the client stamps both within moments, and two real messages are never this close. */
const STAND_IN_WINDOW_MS = 2_000

const stampedNear = (a?: string, b?: string) =>
    Math.abs(Date.parse(a ?? '') - Date.parse(b ?? '')) < STAND_IN_WINDOW_MS

/**
 * A stand-in request/reply/abort is covered once the stored history holds an entry of its kind.
 * A request is matched by content when the person typed it, and by when it was made otherwise: a
 * voice message or a generative-UI event is stored as something other than the `chat` text drawn
 * for it, and a stand-in left uncovered ends up below the reply it started.
 */
function coveredByStored(entry: ChatMessage, stored: ChatMessage[]): boolean {
    if (entry.content.Request) {
        const request = entry.content.Request
        const text = JSON.stringify(request.content)
        return stored.some(
            (s) =>
                s.content.Request &&
                (JSON.stringify(s.content.Request.content) === text || stampedNear(s.content.Request.timestamp, request.timestamp))
        )
    }
    if (entry.content.Response) {
        const ts = entry.content.Response.timestamp
        return stored.some((s) => s.content.Response && s.content.Response.timestamp === ts)
    }
    if (entry.content.Command !== undefined) {
        // The person's abort is drawn the moment it is pressed; the server stores a `Command`
        // entry of its own a moment later, and that one replaces the drawn copy.
        return stored.some(
            (s) => s.content.Command === entry.content.Command && stampedNear(s.timestamp, entry.timestamp)
        )
    }
    return entry.content.Checkpoint !== undefined && stored.some((s) => s.content.Checkpoint !== undefined)
}

/**
 * The newest stored page laid over what is held, in the order the person already sees.
 *
 * The page can predate what is held: a read started when one turn ended can land after the
 * next turn's reply, and laying it over the entries — rather than placing the page before a
 * slice of them and appending the rest — is what keeps that reply below the message it
 * answers. An entry the page carries takes the page's copy where it stands; a stand-in the
 * page covers gives way to it in place; a stand-in it does not cover stays where it is; and an
 * entry nothing holds yet is placed beside the nearest entry of the page that is held, so a
 * `Request` the page brings lands above its own steps instead of after them.
 */
function mergePage(current: ChatMessage[], stored: ChatMessage[]): ChatMessage[] {
    const byUid = new Map(stored.map((entry) => [entry.uid, entry]))
    const out: ChatMessage[] = []
    const shows = (uid: string) => out.some((entry) => entry.uid === uid)

    for (const entry of current) {
        const fresh = byUid.get(entry.uid)
        if (fresh) {
            out.push(fresh)
            continue
        }
        if (isLive(entry)) {
            const cover = stored.find((s) => coveredByStored(entry, [s]))
            if (cover) {
                if (!shows(cover.uid)) out.push(cover)
                continue
            }
        }
        out.push(entry)
    }

    for (let at = 0; at < stored.length; at += 1) {
        const entry = stored[at]
        if (shows(entry.uid)) continue
        // Beside the nearest entry of the page that is here: after the one before it, or
        // before the one after it. A page sharing nothing with what is held goes after it all.
        const preceding = stored.slice(0, at).reverse().find((s) => shows(s.uid))
        const anchor = preceding ?? stored.slice(at + 1).find((s) => shows(s.uid))
        const found = anchor ? out.findIndex((e) => e.uid === anchor.uid) : -1
        out.splice(found < 0 ? out.length : preceding ? found + 1 : found, 0, entry)
    }

    return out
}

/** The next queued message to send, once the topic is idle. */
export function nextQueued(state: TopicState): QueuedMessage | undefined {
    return state.phase === 'idle' ? state.queued[0] : undefined
}
