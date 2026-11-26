// Small, pure readers over history entries and requests for the chat screens.

import type {
    ChatMessage,
    VizierAttachment,
    VizierRequestContent,
    VizierResponseContent,
    VizierResponseStats,
} from '../api/frames'
import { abbreviate, cost as formatCost, count, duration } from './format'
import { estimateCost, type Price } from './prices'
import { zoned } from './timezone'

/** What a person typed (or a stand-in for what they did), for their bubble. */
export function requestText(content: VizierRequestContent): string {
    if ('chat' in content) return content.chat
    if ('client_event' in content) return content.client_event.text
    if ('question_answer' in content) return content.question_answer.text ?? ''
    if ('audio_chat' in content) return content.audio_chat[1] ?? ''
    if ('silent_read' in content) return content.silent_read
    if ('prompt' in content) return content.prompt
    if ('unattended' in content) return content.unattended
    if ('command' in content) return `/${content.command}`
    return ''
}

/** A voice message: the audio is the request's own content, not one of its `attachments`. */
export function isAudioRequest(
    content: VizierRequestContent
): content is Extract<VizierRequestContent, { audio_chat: unknown }> {
    return 'audio_chat' in content
}

/**
 * Everything a request carries to draw, the voice message included.
 */
export function requestAttachments(
    content: VizierRequestContent,
    attachments: VizierAttachment[] | undefined
): VizierAttachment[] {
    return isAudioRequest(content) ? [content.audio_chat[0], ...(attachments ?? [])] : (attachments ?? [])
}

/**
 * The voice message a person sent: its recording and the words heard in it. Nothing else in the
 * message has words: a file uploaded alongside a voice message is a file, not a transcript.
 */
export function requestSpoken(content: VizierRequestContent): Spoken | null {
    if (!isAudioRequest(content)) return null
    return { attachment: content.audio_chat[0], transcript: content.audio_chat[1] }
}

/** A pre-058 background job report, read from old history only (#82). */
export const isBackgroundReport = (c: VizierRequestContent): c is Extract<VizierRequestContent, { background_report: unknown }> =>
    'background_report' in c

/** Finished detached scripts, delivered to the agent (spec 058). */
export const isDetachedDelivery = (c: VizierRequestContent): c is Extract<VizierRequestContent, { detached_delivery: string }> =>
    'detached_delivery' in c

export function replyText(content: VizierResponseContent): string | null {
    if (typeof content !== 'object' || content === null) return null
    if ('message' in content) return content.message.content
    if ('audio_reply' in content) return content.audio_reply[1]
    return null
}

/**
 * What was spoken: the recording and the words in it. A transcript belongs to one recording,
 * never to a whole message: a file the agent attached carries no words of its own, so the
 * reply it wrote is not read back to it as one.
 */
export interface Spoken {
    /** The recording the words belong to. */
    attachment: VizierAttachment
    /** The words, when they are known: null while a live voice message is still recognised. */
    transcript: string | null
}

/** A reply the agent spoke: the audio is its own content, not one of the `attachments`. */
export function isAudioReply(
    content: VizierResponseContent | undefined
): content is Extract<VizierResponseContent, { audio_reply: unknown }> {
    return typeof content === 'object' && content !== null && 'audio_reply' in content
}

/**
 * Everything a reply carries to draw. A spoken reply's audio lives in its content, so the
 * attachment list alone is empty and the player would never appear.
 */
export function replyAttachments(
    content: VizierResponseContent | undefined,
    attachments: VizierAttachment[] | undefined
): VizierAttachment[] {
    return isAudioReply(content) ? [content.audio_reply[0], ...(attachments ?? [])] : (attachments ?? [])
}

/**
 * The reply the agent spoke: its recording and the words said in it. A plain message reply has
 * none: the prose it wrote is the reply, not a transcript of some file it attached.
 */
export function replySpoken(content: VizierResponseContent | undefined): Spoken | null {
    if (!isAudioReply(content)) return null
    return { attachment: content.audio_reply[0], transcript: content.audio_reply[1] }
}

export function replyStats(content: VizierResponseContent): VizierResponseStats | undefined {
    if (typeof content !== 'object' || content === null) return undefined
    if ('message' in content) return content.message.stats
    if ('audio_reply' in content) return content.audio_reply[2] ?? undefined
    return undefined
}

export function replyError(content: VizierResponseContent): { kind: string; message: string } | null {
    return typeof content === 'object' && content !== null && 'error' in content ? content.error : null
}

/**
 * "anthropic/claude-sonnet-5-5 · 6.1k in · 712 out · ~$0.03". `totalMs` adds the turn's whole
 * time, for a turn in several parts, whose trails each show only their own.
 */
export function statsLine(stats: VizierResponseStats & { provider?: string; model?: string }, price?: Price | null, totalMs?: number): string {
    const parts: string[] = []
    if (stats.model) parts.push(stats.provider ? `${stats.provider}/${stats.model}` : stats.model)
    parts.push(`${abbreviate(stats.total_input_tokens)} in`, `${abbreviate(stats.total_output_tokens)} out`)
    if (price) parts.push(formatCost(estimateCost(stats, price)))
    if (totalMs !== undefined) parts.push(duration(totalMs))
    return parts.join(' · ')
}

export const time = (timestamp?: string) =>
    timestamp ? zoned(timestamp).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) : ''

export const tokens = count

export function isUserEntry(entry: ChatMessage): boolean {
    return entry.content.Request !== undefined
}
