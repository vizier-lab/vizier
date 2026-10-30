/**
 * The side chat's one question and answer, as pure data: no store, no storage. The window
 * holds one of these in `useReducer` and drops it when it closes (spec 033, research R8). The
 * server also accepts earlier turns and a 32,000-character budget for the clients that keep
 * follow-ups (Discord, Telegram, the API); this window is throwaway and sends neither.
 */

/** A question is 1–4,000 characters (the server's limit). */
export const MAX_QUESTION_CHARS = 4_000

export type SideChatTurn = {
    question: string
    /** `null` while the answer is pending or after it failed. */
    answer: string | null
    /** Set when the ask failed; a retry asks the same question again. */
    error?: { message: string; retryable: boolean }
}

export type SideChatState = {
    turns: SideChatTurn[]
    /** Index of the turn being asked, if any. One ask at a time. */
    pending: number | null
}

export const initialSideChat: SideChatState = { turns: [], pending: null }

export type SideChatAction =
    | { type: 'ask'; question: string }
    | { type: 'answered'; index: number; answer: string }
    | { type: 'failed'; index: number; message: string; retryable: boolean }
    | { type: 'retry'; index: number }
    | { type: 'reset' }

/** Why a question can't be asked, or `null` when it can. */
export function refusal(question: string): string | null {
    const text = question.trim()
    if (text.length === 0) return 'Type a question first.'
    if (text.length > MAX_QUESTION_CHARS)
        return `Questions are limited to ${MAX_QUESTION_CHARS.toLocaleString()} characters.`
    return null
}

export function sideChatReducer(state: SideChatState, action: SideChatAction): SideChatState {
    switch (action.type) {
        case 'ask':
            return {
                turns: [...state.turns, { question: action.question.trim(), answer: null }],
                pending: state.turns.length,
            }
        case 'answered':
            if (state.pending !== action.index) return state;
            return {
                turns: state.turns.map((t, i) =>
                    i === action.index ? { ...t, answer: action.answer, error: undefined } : t
                ),
                pending: state.pending === action.index ? null : state.pending,
            }
        case 'failed':
            if (state.pending !== action.index) return state;
            return {
                turns: state.turns.map((t, i) =>
                    i === action.index
                        ? { ...t, error: { message: action.message, retryable: action.retryable } }
                        : t
                ),
                pending: state.pending === action.index ? null : state.pending,
            }
        case 'retry':
            return {
                turns: state.turns.map((t, i) => (i === action.index ? { ...t, error: undefined } : t)),
                pending: action.index,
            }
        case 'reset':
            return initialSideChat
    }
}

/** What a failed ask shows, from the HTTP status and the server's message (contracts/http-btw.md). */
export function mapAskError(status: number | undefined, message: string): { message: string; retryable: boolean } {
    if (status === 429) {
        const secs = /(\d+)s/.exec(message)?.[1]
        return {
            message: secs ? `Slow down — try again in ${secs}s` : 'Slow down — try again shortly',
            retryable: true,
        }
    }
    if (status === 422) return { message, retryable: false }
    if (status === 403 || status === 404) return { message, retryable: false }
    // 408, 5xx, network failures: the same question may succeed later.
    return { message: message || 'Something went wrong — try again', retryable: true }
}
