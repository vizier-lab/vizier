// What of a saved conversation may leave, as one table (research R2). A kind that is not listed is
// excluded, so adding a history variant can never leak by default.
//   trail-off : in the snapshot whether or not the person switched the activity trail on
//   trail-on  : in the snapshot only with the trail on (cleaned)
//   never     : never in the snapshot
export type Disposition = 'trail-off' | 'trail-on' | 'never'

/** One row per `SessionHistoryContent` variant (the key of `ChatMessage.content`). */
export const HISTORY_KINDS: Record<string, Disposition> = {
    Request: 'trail-off',
    Response: 'trail-off', // refined per content by `responseDisposition`
    AssistantMessage: 'trail-off',
    ClientToolCall: 'trail-on', // `ui_render` is refined to trail-off by `clientCallDisposition`
    ToolCall: 'trail-on',
    ToolResult: 'trail-on',
    QuestionSet: 'trail-off',
    SensitiveAsk: 'never',
    Recall: 'never',
    Checkpoint: 'never',
    Command: 'never',
}

/** Request contents shown as what a person said. Everything else (silent reads, unattended prompts, background reports, commands) is internal. */
export const REQUEST_KINDS: Record<string, Disposition> = {
    chat: 'trail-off',
    client_event: 'trail-off',
    question_answer: 'trail-off',
    audio_chat: 'trail-off',
}

/** Response contents. Reasoning is never shared; live-frame shapes of tool use belong to the trail. */
export const RESPONSE_KINDS: Record<string, Disposition> = {
    message: 'trail-off',
    thinking: 'never',
    thinking_start: 'never',
    tool_choice: 'trail-on',
    tool_response: 'trail-on',
    checkpoint: 'never',
    audio_reply: 'never',
    error: 'never',
    empty: 'never',
    abort: 'never',
}

export const UI_RENDER = 'ui_render'

export const dispositionOf = (table: Record<string, Disposition>, kind: string): Disposition =>
    Object.prototype.hasOwnProperty.call(table, kind) ? table[kind] : 'never'

/** Whether something with this disposition is in a snapshot made with the trail on or off. */
export const included = (d: Disposition, includeTrail: boolean): boolean => d === 'trail-off' || (d === 'trail-on' && includeTrail)

export function requestDisposition(content: unknown): Disposition {
    return dispositionOf(REQUEST_KINDS, variantOf(content))
}

export function responseDisposition(content: unknown): Disposition {
    return dispositionOf(RESPONSE_KINDS, variantOf(content))
}

/** The disposition of a whole history entry's `content` (its first key). */
export function entryDisposition(content: Record<string, unknown> | undefined | null): { kind: string; disposition: Disposition } {
    const kind = variantOf(content)
    if (!kind) return { kind: '', disposition: 'never' }
    const base = dispositionOf(HISTORY_KINDS, kind)
    if (kind === 'Request') return { kind, disposition: base === 'never' ? 'never' : requestDisposition((content as Record<string, { content?: unknown }>)[kind]?.content) }
    if (kind === 'Response') return { kind, disposition: base === 'never' ? 'never' : responseDisposition((content as Record<string, { content?: unknown }>)[kind]?.content) }
    if (kind === 'ClientToolCall') {
        const call = (content as Record<string, { name?: string; tool?: string }>)[kind]
        return { kind, disposition: call?.name === UI_RENDER ? 'trail-off' : base }
    }
    return { kind, disposition: base }
}

/** The name of the single variant an externally tagged value holds, or a bare string variant. */
export function variantOf(value: unknown): string {
    if (typeof value === 'string') return value
    if (value && typeof value === 'object') return Object.keys(value as object)[0] ?? ''
    return ''
}
