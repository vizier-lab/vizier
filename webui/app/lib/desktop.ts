import type { DesktopState } from '../api/transport'

/** What the window shows for each engine state: the app itself, or a bootstrap screen. */
export type BootstrapView = 'app' | 'starting' | 'locked' | 'needs_owner' | 'choose_owner' | 'failed'

export function bootstrapView(state: DesktopState | null): BootstrapView {
    if (!state) return 'starting'
    switch (state.state) {
        case 'ready':
        case 'degraded':
            return 'app'
        default:
            return state.state
    }
}

/** The subsystems named by a degraded state's detail, for the banner. */
export function stoppedSubsystems(state: DesktopState | null): string[] {
    if (state?.state !== 'degraded') return []
    const d = state.detail as { stopped?: unknown } | undefined
    return Array.isArray(d?.stopped) ? d.stopped.filter((x): x is string => typeof x === 'string') : []
}

export type OwnerCandidate = { user_id: string; username: string }

export function ownerCandidates(state: DesktopState | null): OwnerCandidate[] {
    const d = state?.detail as { candidates?: unknown } | undefined
    if (!Array.isArray(d?.candidates)) return []
    return d.candidates.filter((c): c is OwnerCandidate => !!c && typeof (c as OwnerCandidate).user_id === 'string')
}

export function lockHolder(state: DesktopState | null): string | undefined {
    const d = state?.detail as { holder?: unknown } | undefined
    return typeof d?.holder === 'string' ? d.holder : undefined
}

export function failureMessage(state: DesktopState | null): string | undefined {
    const d = state?.detail as { error?: unknown } | undefined
    return typeof d?.error === 'string' ? d.error : undefined
}

/** The code of a rejected desktop command (the shell rejects with `{code, message}`). */
export function errorCode(e: unknown): string | undefined {
    return e && typeof e === 'object' && typeof (e as { code?: unknown }).code === 'string' ? (e as { code: string }).code : undefined
}

export function errorText(e: unknown, fallback: string): string {
    if (e instanceof Error) return e.message
    if (e && typeof e === 'object' && typeof (e as { message?: unknown }).message === 'string') return (e as { message: string }).message
    return typeof e === 'string' ? e : fallback
}

/** A notification goes to the OS only when the person isn't looking at the window. */
export function shouldNotifyOs(visibility: string, focused: boolean): boolean {
    return visibility === 'hidden' || !focused
}

/** Poll every 15s while visible; the desktop app keeps going at 60s while hidden. */
export function pollDelay(desktop: boolean, visibility: string): number | null {
    if (visibility === 'visible') return 15_000
    return desktop ? 60_000 : null
}

/** Settings are valid to save: a port in range. */
export function validPort(port: number): boolean {
    return Number.isInteger(port) && port >= 1 && port <= 65535
}
