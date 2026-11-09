/**
 * ARCHIVED (C11): the previous surface, no longer rendered.
 *
 * A live session is one row inside the composer's meta line now, and `LiveVoiceControl` renders
 * that row instead of this bar and its button. Both files stay in the tree because the handoff
 * geometry and the review notes still refer to them; nothing imports them.
 *
 * What follows is the bar as it was, including its interrupt control — which FR-020/FR-021 have
 * since deferred (C7). Do not read it as behaviour this release ships.
 */
/**
 * The live session's status line and its interrupt control (`specs/055-live-voice` FR-020, FR-038,
 * FR-042).
 *
 * Two things are deliberate here. The thinking indicator is its own wording and its own colour, so
 * "the agent is thinking" can never be read as "the connection dropped". And the interrupt control
 * is present for the whole session: a person never has to wait for the agent to start talking
 * before they can stop it.
 */

import type { LiveVoiceView } from '../../lib/liveVoice/session'

export interface LiveVoiceBarProps {
    view: LiveVoiceView
    onInterrupt(): void
}

/** One wording per state, and the thinking state is never `reconnecting`'s wording. */
export function stateWord(view: LiveVoiceView): string {
    if (view.state === 'waiting') return 'agent sedang berpikir'
    switch (view.state) {
        case 'connecting':
            return 'connecting'
        case 'listening':
            return 'listening'
        case 'speaking':
            return 'speaking'
        case 'interrupted':
            return 'interrupted'
        case 'reconnecting':
            return 'reconnecting'
        case 'ended':
            return 'ended'
        case 'failed':
            return 'failed'
        default:
            return 'off'
    }
}

function seconds(ms: number): string {
    return `${Math.floor(ms / 1000)}s`
}

export function LiveVoiceBar({ view, onInterrupt }: LiveVoiceBarProps) {
    const live = view.state !== 'off' && view.state !== 'ended' && view.state !== 'failed'
    if (!live) return null
    // FR-042: thinking and reconnecting must not share a look.
    const tone =
        view.state === 'waiting'
            ? 'lv-bar-thinking'
            : view.state === 'reconnecting'
              ? 'lv-bar-reconnecting'
              : view.state === 'speaking'
                ? 'lv-bar-speaking'
                : 'lv-bar-listening'
    return (
        <div className={`lv-bar ${tone}`} role="status" aria-live="polite">
            <span className="lv-state">{stateWord(view)}</span>
            {view.thinking && <span className="lv-elapsed">· {seconds(view.thinkingMs)}</span>}
            {view.micOpen && (
                <span className="lv-mic" title="the microphone is open">
                    ● mic
                </span>
            )}
            <span className="lv-level" aria-hidden>
                <span className="lv-level-fill" style={{ width: `${Math.round(view.level * 100)}%` }} />
            </span>
            {view.transcript && <span className="lv-transcript">“{view.transcript}”</span>}
            {view.notice && <span className="lv-notice">{view.notice}</span>}
            <button
                type="button"
                className="lv-interrupt"
                // Never hidden inside a live session: only disabled when there is nothing to do.
                disabled={view.state === 'connecting' || view.state === 'reconnecting'}
                onClick={onInterrupt}
            >
                Interrupt
            </button>
        </div>
    )
}
