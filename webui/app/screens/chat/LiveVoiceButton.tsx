/**
 * ARCHIVED (C11): the previous on/off control, no longer rendered.
 *
 * The row's idle pill is the control now. Kept for the geometry notes, not for use.
 */
/**
 * The one control that turns live voice on and off (`specs/055-live-voice` FR-001, FR-002, FR-003).
 *
 * Separate from the microphone button and the spoken-reply toggle: this is its own feature, with
 * its own switch, and it is off on every visit — nothing is remembered between them (FR-035).
 */

import type { SessionState } from '../../lib/liveVoice/frames'

export interface LiveVoiceButtonProps {
    /** The agent detail's `live_voice`: on only when the switch is on and both halves exist. */
    capability: boolean
    state: SessionState
    /** Which half is missing, when the capability is false (FR-004). */
    missing?: 'recognizer' | 'voice' | null
    onStart(): void
    onStop(): void
    /** Opens the agent's speech settings, for the disabled case. */
    onOpenSettings?(): void
}

export function LiveVoiceButton({ capability, state, missing, onStart, onStop, onOpenSettings }: LiveVoiceButtonProps) {
    const live = state !== 'off' && state !== 'ended' && state !== 'failed'
    const label = live ? 'Stop live voice' : 'Live voice'

    if (!capability) {
        // FR-004: say which half is missing and where to set it, not a bare tooltip.
        const reason =
            missing === 'voice'
                ? 'a voice is not configured for this agent'
                : missing === 'recognizer'
                  ? 'a recogniser is not configured for this agent'
                  : 'live voice is not configured for this agent'
        return (
            <button type="button" className="lv-button" disabled title={reason} onClick={onOpenSettings}>
                <span aria-hidden>🎙</span>
                <span className="lv-button-label">Live voice</span>
                <span className="lv-button-why">{reason}</span>
            </button>
        )
    }

    return (
        <button
            type="button"
            className={live ? 'lv-button lv-button-on' : 'lv-button'}
            aria-pressed={live}
            aria-label={label}
            title={live ? 'Stop the live conversation' : 'Talk to the agent out loud'}
            onClick={live ? onStop : onStart}
        >
            <span aria-hidden>{live ? '◼' : '🎙'}</span>
            <span className="lv-button-label">{label}</span>
        </button>
    )
}
