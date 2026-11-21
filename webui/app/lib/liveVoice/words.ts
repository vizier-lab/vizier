/**
 * How a live voice session's state is shown (spec 055 FR-042). `waiting` (the agent working) reads as
 * listening in the pill, and the activity trail shows the work; it is never drawn like `reconnecting`.
 * `off` is no call at all. The words themselves are design's
 * (`callWord`), so the pill and anything that names a state read the same.
 */

import { callWord, type CallState } from '@vizier/design'
import type { SessionState } from './frames'

/** The pill's state for a session state; `null` while there is no call. */
export function callState(state: SessionState): CallState | null {
    if (state === 'off') return null
    // The agent working is the conversation's activity trail to show, not the call's: while it
    // works the call is still listening (speaking over it interrupts), so the pill says so.
    if (state === 'waiting') return 'listening'
    return state
}

/** The word a session state is read as: "Thinking", "Reconnecting", "Call ended". */
export function stateWord(view: { state: SessionState }): string {
    const state = callState(view.state)
    return state ? callWord(state) : 'Live voice'
}
