// Small, pure readers over history entries and requests for the chat screens.

import type {
    ChatMessage,
    VizierAttachment,
    VizierRequestContent,
    VizierResponseContent,
    VizierResponseStats,
} from '../api/frames'
import { abbreviate, cost as formatCost, count } from './format'
import { estimateCost, type Price } from './prices'

/** What a person typed (or a stand-in for what they did), for their bubble. */
export function requestText(content: VizierRequestContent): string {
    if ('chat' in content) return content.chat
    if ('client_event' in content) return content.client_event.text
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

/** Everything a request carries to draw, the voice message included. */
export function requestAttachments(
    content: VizierRequestContent,
    attachments: VizierAttachment[] | undefined
): VizierAttachment[] {
    return isAudioRequest(content) ? [content.audio_chat[0], ...(attachments ?? [])] : (attachments ?? [])
}

export const isBackgroundReport = (c: VizierRequestContent): c is Extract<VizierRequestContent, { background_report: unknown }> =>
    'background_report' in c

export function replyText(content: VizierResponseContent): string | null {
    if (typeof content !== 'object' || content === null) return null
    if ('message' in content) return content.message.content
    if ('audio_reply' in content) return content.audio_reply[1]
    return null
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

export function replyStats(content: VizierResponseContent): VizierResponseStats | undefined {
    if (typeof content !== 'object' || content === null) return undefined
    if ('message' in content) return content.message.stats
    if ('audio_reply' in content) return content.audio_reply[2] ?? undefined
    return undefined
}

export function replyError(content: VizierResponseContent): { kind: string; message: string } | null {
    return typeof content === 'object' && content !== null && 'error' in content ? content.error : null
}

/** "anthropic/claude-sonnet-5-5 · 6.1k in · 712 out · ~$0.03" */
export function statsLine(stats: VizierResponseStats & { provider?: string; model?: string }, price?: Price | null): string {
    const parts: string[] = []
    if (stats.model) parts.push(stats.provider ? `${stats.provider}/${stats.model}` : stats.model)
    parts.push(`${abbreviate(stats.total_input_tokens)} in`, `${abbreviate(stats.total_output_tokens)} out`)
    if (price) parts.push(formatCost(estimateCost(stats, price)))
    return parts.join(' · ')
}

export const time = (timestamp?: string) =>
    timestamp ? new Date(timestamp).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) : ''

export const tokens = count

export function isUserEntry(entry: ChatMessage): boolean {
    return entry.content.Request !== undefined
}
