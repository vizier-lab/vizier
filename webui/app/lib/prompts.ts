// The prompt ruler's model: every message the person sent in a conversation, in order, whether
// its page of history is loaded or not. The server's list (`GET …/prompts`) covers what is
// stored; the entries on the page add what is not yet (a message just sent).

import type { ChatMessage, VizierRequestContent } from '../api/frames'
import { requestText } from './messages'
import { zoned } from './timezone'

/** One prompt, as the server lists it. */
export interface PromptMark {
    uid: string
    seq?: number | null
    timestamp: string
    preview: string
}

/** The request kinds a person sends: what they typed, spoke, answered, or did on a page. */
export function isPrompt(content: VizierRequestContent): boolean {
    return 'chat' in content || 'prompt' in content || 'audio_chat' in content || 'client_event' in content || 'question_answer' in content
}

/** The first line that says something, as the server cuts it. */
export function previewOf(text: string, max = 120): string {
    const line = text.split('\n').map((l) => l.trim()).find((l) => l !== '') ?? ''
    return line.length <= max ? line : `${line.slice(0, max - 1).trimEnd()}…`
}

/** A loaded entry as a mark, when it is a person's prompt. */
export function markOf(entry: ChatMessage): PromptMark | null {
    const request = entry.content.Request
    if (!request || !isPrompt(request.content)) return null
    const text = 'audio_chat' in request.content ? (request.content.audio_chat[1] || 'Voice message') : requestText(request.content)
    const files = request.attachments?.length ?? 0
    const preview = previewOf(text) || (files === 0 ? '' : files === 1 ? '1 attachment' : `${files} attachments`)
    return { uid: entry.uid, seq: entry.seq ?? null, timestamp: entry.timestamp ?? request.timestamp ?? '', preview }
}

/**
 * The server's list with the loaded prompts it does not have yet (sent since it was read, or a
 * stand-in still waiting to be stored) added after it. A stand-in the server has since stored
 * is not counted twice: once the stored copy is loaded, the stand-in is gone from the entries.
 */
export function promptIndex(server: PromptMark[], entries: ChatMessage[]): PromptMark[] {
    const known = new Set(server.map((m) => m.uid))
    // Compared as instants: the server writes its own offset (`+07:00`), a stand-in is UTC (`Z`).
    const newest = server.length ? Date.parse(server[server.length - 1].timestamp) : -Infinity
    const extra = entries
        .map(markOf)
        .filter((m): m is PromptMark => m !== null && !known.has(m.uid) && Date.parse(m.timestamp) >= newest)
    return extra.length ? [...server, ...extra] : server
}

/** Messages sent while the agent was busy, drawn as waiting: prompts already, last in order. */
export function withWaiting(index: PromptMark[], waiting: { id: string; text: string; sentAt?: string }[]): PromptMark[] {
    if (!waiting.length) return index
    return [
        ...index,
        ...waiting.map((q) => ({ uid: waitingUid(q.id), seq: null, timestamp: q.sentAt ?? '', preview: previewOf(q.text) })),
    ]
}

/** The `data-prompt-uid` a waiting message is drawn with. */
export const waitingUid = (id: string) => `q-${id}`

/**
 * The prompt being read: the last one whose top is above the reading line, else the first
 * loaded one (the reader is above it, in older history or at the very top).
 * `tops` maps each loaded prompt's uid to its top, measured from the reading line (negative is
 * above it).
 */
export function currentPrompt(index: PromptMark[], tops: Map<string, number>): number {
    let current = -1
    let firstLoaded = -1
    index.forEach((mark, i) => {
        const top = tops.get(mark.uid)
        if (top === undefined) return
        if (firstLoaded < 0) firstLoaded = i
        if (top <= 0) current = i
    })
    if (current >= 0) return current
    return firstLoaded >= 0 ? firstLoaded : Math.max(0, index.length - 1)
}

/** "21:24" today, "14 Nov 21:24" another day, in the server's zone. */
export function markTime(timestamp: string, now: Date | string = new Date()): string {
    const d = zoned(timestamp)
    const n = zoned(now)
    const hm = d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
    const sameDay = d.getFullYear() === n.getFullYear() && d.getMonth() === n.getMonth() && d.getDate() === n.getDate()
    return sameDay ? hm : `${d.getDate()} ${d.toLocaleString('en-GB', { month: 'short' })} ${hm}`
}
