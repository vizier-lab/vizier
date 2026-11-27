// Connecting a Discord or Telegram account with a one-time code (spec 069). Pure: the screens
// are in `screens/settings/ConnectPanel.tsx` and `routes/connect.tsx`.
import type { Platform } from '../api/endpoints/connections'
import type { VizierRequestMessage } from '../api/frames'
import { ApiError } from '../api/transport/common'
import { personName } from './tasks'
import { zoned } from './timezone'

export const CODE_LENGTH = 8

/** What was typed, as the code it means: case, spaces and dashes don't matter. */
export function normalizeCode(input: string): string {
    return input.replace(/[\s-]/g, '').toUpperCase()
}

/** Long enough to be worth asking the server about; the server decides whether it is real. */
export function isCodeShaped(input: string): boolean {
    return /^[A-Z0-9]{8}$/.test(normalizeCode(input))
}

/** The code a `/connect#CODE` link carries, if it carries one. */
export function codeFromHash(hash: string): string | null {
    let text = hash.replace(/^#/, '')
    try {
        text = decodeURIComponent(text)
    } catch {
        return null
    }
    return isCodeShaped(text) ? normalizeCode(text) : null
}

const STASH_KEY = 'vizier.connect.code'

// The code waits in this tab while the person signs in, so it never rides in a URL the server
// sees. It also lives in memory, for a browser that blocks storage.
let held: string | null = null

export function stashCode(code: string) {
    held = code
    try {
        sessionStorage.setItem(STASH_KEY, code)
    } catch {
        /* storage blocked: the code lasts until a reload */
    }
}

export function stashedCode(): string | null {
    try {
        return sessionStorage.getItem(STASH_KEY) ?? held
    } catch {
        return held
    }
}

export function clearStashedCode() {
    held = null
    try {
        sessionStorage.removeItem(STASH_KEY)
    } catch {
        /* nothing was kept */
    }
}

export type Refusal = { state: 'unavailable' | 'elsewhere' | 'failed'; message: string }

/** Which state the confirmation shows for a refused preview or confirm. */
export function refusalOf(error: unknown): Refusal {
    const message = error instanceof Error ? error.message : 'Something went wrong.'
    const code = error instanceof ApiError ? error.code : undefined
    if (code === 'connect_code_unavailable') return { state: 'unavailable', message }
    if (code === 'platform_linked_elsewhere') return { state: 'elsewhere', message }
    return { state: 'failed', message }
}

export const platformName = (platform: Platform) => (platform === 'discord' ? 'Discord' : 'Telegram')

/** A handle without the "@" a person may have typed before it. */
export function bareHandle(handle: string | null | undefined): string | undefined {
    return handle?.trim().replace(/^@+/, '') || undefined
}

/** "@rin_dev", or "account 55120" for one without a handle. */
export function accountName(account: { platform_username?: string | null; platform_user_id: string }): string {
    const handle = bareHandle(account.platform_username)
    return handle ? `@${handle}` : `account ${account.platform_user_id}`
}

/** "Connected 27 Nov 2026", on the instance's clock. */
export function connectedSince(at: string): string {
    const day = zoned(at).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })
    return `Connected ${day}`
}

export type ShownSender = {
    name: string
    /** What the avatar's face is drawn from. */
    seed: string
    /** Absent for a message saved before accounts could be connected, or not from a platform. */
    sender?: 'connected' | 'unregistered'
}

const text = (value: unknown) => (typeof value === 'string' && value !== '' ? value : undefined)

/**
 * Who a message in a Discord or Telegram conversation is shown as. A connected account reads as
 * its Vizier user; an unconnected one by what the platform calls it, marked as unregistered, so
 * two strangers can still be told apart.
 */
export function shownSender(request: VizierRequestMessage): ShownSender {
    if (request.verified === true) return { name: request.user, seed: request.user, sender: 'connected' }
    if (request.verified === false) {
        const metadata = request.metadata ?? {}
        const name = text(metadata.platform_display_name) ?? text(metadata.platform_username) ?? request.user
        return { name, seed: text(metadata.platform_user_id) ?? name, sender: 'unregistered' }
    }
    return { name: personName(request.user), seed: request.user }
}
