/**
 * Which half of a live voice session is missing (spec 055, FR-004).
 *
 * The server sends this so the pill can name what is absent. It chooses **a sentence**, never
 * readiness: `live_voice` is what says whether a call may start. `null` means "nothing to name" —
 * either the agent's switch is off or both halves exist — so it must never be read as "ready".
 */
export type LiveVoiceMissing = 'recognizer' | 'voice'

/** The two halves as the server spells them; anything else is a word we cannot render. */
const MISSING: readonly LiveVoiceMissing[] = ['recognizer', 'voice']

export function liveVoiceMissingFrom(
    detail: { live_voice_missing?: LiveVoiceMissing | null } | null | undefined
): LiveVoiceMissing | null {
    const missing = detail?.live_voice_missing
    // A newer server may name something this build has no sentence for. Passing it through would
    // promise a sentence the row does not have, so it reads as nothing to name.
    return MISSING.includes(missing as LiveVoiceMissing) ? (missing as LiveVoiceMissing) : null
}
