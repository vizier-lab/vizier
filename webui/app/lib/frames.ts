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
}

export type Action =
    | { type: 'frame'; frame: Frame; now?: string }
    | { type: 'history'; entries: ChatMessage[]; position?: 'latest' | 'older' }
    | { type: 'busy'; thinking: boolean }
    | { type: 'send'; id: string; text: string; attachments?: VizierAttachment[]; spoken?: boolean; now?: string }
    | { type: 'dequeue'; now?: string }
    | { type: 'abort' }
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
                    return {
                        ...state,
                        phase: busy ? 'working' : state.phase,
                        entries: mergeEntries(state.entries, [frame.entry]),
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
                case 'client': {
                    const f = frame.frame
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
            // newest page replaces its own span of it.
            const stored = action.entries
            const held = state.entries.filter((e) => !isLive(e))
            const keptLive = state.entries.filter(
                (e) => isLive(e) && !stored.some((s) => s.uid === e.uid) && !coveredByStored(e, stored)
            )
            let merged: ChatMessage[]
            if (action.position === 'older') {
                const have = new Set(held.map((e) => e.uid))
                merged = [...stored.filter((e) => !have.has(e.uid)), ...held]
            } else {
                const first = held.findIndex((e) => stored.some((s) => s.uid === e.uid))
                const before = first < 0 ? held : held.slice(0, first)
                const inStored = new Set(stored.map((e) => e.uid))
                // Pushed after the page was read: not in it yet, so kept after it.
                const newer = first < 0 ? [] : held.slice(first).filter((e) => !inStored.has(e.uid) && !stored.some((s) => s.uid === e.uid))
                merged = [...before, ...stored, ...newer]
            }
            const context = state.context ?? (action.position === 'older' ? null : contextOfHistory(stored))
            return { ...state, context, entries: keptLive.length ? [...merged, ...keptLive] : merged }
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

        case 'abort':
            return { ...idle(state), queued: [], stopped: true }

        case 'files':
            return { ...state, files: action.files }

        case 'jobs':
            return { ...state, jobs: Object.fromEntries(action.jobs.map((j) => [j.id, j])) }
    }
}

/** A stand-in request/reply is covered once the stored history holds an entry of its kind after it. */
function coveredByStored(entry: ChatMessage, stored: ChatMessage[]): boolean {
    if (entry.content.Request) {
        const text = JSON.stringify(entry.content.Request.content)
        return stored.some((s) => s.content.Request && JSON.stringify(s.content.Request.content) === text)
    }
    if (entry.content.Response) {
        const ts = entry.content.Response.timestamp
        return stored.some((s) => s.content.Response && s.content.Response.timestamp === ts)
    }
    return entry.content.Checkpoint !== undefined && stored.some((s) => s.content.Checkpoint !== undefined)
}

/** The next queued message to send, once the topic is idle. */
export function nextQueued(state: TopicState): QueuedMessage | undefined {
    return state.phase === 'idle' ? state.queued[0] : undefined
}
