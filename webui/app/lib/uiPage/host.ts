// The host side of an agent page's frame (specs/063-html-generative-ui contracts/bridge.md): the
// rules a page cannot get around, because they are enforced out here. The frame's own runtime
// is `app/kit/runtime.ts`; both speak `{ vz: 1, type, ... }`.

import { ROLE_VARS } from '../themes/roles'

export const PROTOCOL = 1
/** A send counts as the person's only this long after they last interacted with the frame. */
export const INTERACTION_WINDOW_MS = 10_000
/** At most one send per second. */
export const SEND_INTERVAL_MS = 1_000
/** State saves are coalesced to one per this long (at most twice a second). */
export const STATE_COALESCE_MS = 500
/** No heartbeat for this long and the page is reported unresponsive. */
export const UNRESPONSIVE_MS = 1_000

export type Problem = {
    kind: 'error' | 'rejection' | 'console' | 'blocked' | 'kit' | 'unresponsive'
    message: string
    source: 'page' | 'kit'
    line?: number
    column?: number
    detail?: string
}

/** What a frame may say, once checked. Anything else is ignored. */
export type FrameMessage =
    | { type: 'ready'; pageId: string; version: number }
    | { type: 'heartbeat' }
    | { type: 'height'; px: number }
    | { type: 'problem'; problem: Problem }
    | { type: 'state.set'; value: unknown }
    | { type: 'send'; id: number; text: string; data: unknown }
    | { type: 'files.url'; id: number; name: string }

const isObj = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v)
const KINDS = new Set<Problem['kind']>(['error', 'rejection', 'console', 'blocked', 'kit', 'unresponsive'])
const clip = (v: unknown, n = 500) => String(v ?? '').slice(0, n)

/** Check a message from a page's frame. The caller has already checked it came from that frame. */
export function parseFrameMessage(data: unknown): FrameMessage | null {
    if (!isObj(data) || data.vz !== PROTOCOL || typeof data.type !== 'string') return null
    switch (data.type) {
        case 'ready':
            return { type: 'ready', pageId: clip(data.pageId, 64), version: Number(data.version) || 0 }
        case 'heartbeat':
            return { type: 'heartbeat' }
        case 'height':
            return typeof data.px === 'number' && Number.isFinite(data.px) ? { type: 'height', px: Math.max(0, Math.ceil(data.px)) } : null
        case 'problem': {
            const p = data.problem
            if (!isObj(p) || !KINDS.has(p.kind as Problem['kind'])) return null
            return {
                type: 'problem',
                problem: {
                    kind: p.kind as Problem['kind'],
                    message: clip(p.message),
                    source: p.source === 'kit' ? 'kit' : 'page',
                    ...(typeof p.line === 'number' ? { line: p.line } : {}),
                    ...(typeof p.column === 'number' ? { column: p.column } : {}),
                    ...(p.detail != null ? { detail: clip(p.detail) } : {}),
                },
            }
        }
        case 'state.set':
            return { type: 'state.set', value: data.value ?? null }
        case 'send':
            return typeof data.id === 'number' ? { type: 'send', id: data.id, text: clip(data.text, 4000).trim(), data: data.data } : null
        case 'files.url':
            return typeof data.id === 'number' ? { type: 'files.url', id: data.id, name: clip(data.name, 255) } : null
        default:
            return null
    }
}

/** Who may send, and how often: a real interaction recently, and one per second. */
export class SendGate {
    private lastInteraction = 0
    private lastSend = 0

    interacted(now: number) {
        this.lastInteraction = now
    }

    /** `null` when the send may go, otherwise why not. Records the send when it may. */
    check(now: number, focused: boolean): 'no_interaction' | 'rate_limited' | null {
        const recent = focused || (this.lastInteraction > 0 && now - this.lastInteraction < INTERACTION_WINDOW_MS)
        if (!recent) return 'no_interaction'
        if (this.lastSend > 0 && now - this.lastSend < SEND_INTERVAL_MS) return 'rate_limited'
        this.lastSend = now
        return null
    }
}

/** Coalesces state saves: the last value in each window is the one saved. */
export class StateCoalescer {
    private pending: { value: unknown } | null = null
    private timer: ReturnType<typeof setTimeout> | null = null

    constructor(private readonly save: (value: unknown) => void, private readonly windowMs = STATE_COALESCE_MS) {}

    push(value: unknown) {
        this.pending = { value }
        if (this.timer) return
        this.timer = setTimeout(() => {
            this.timer = null
            const next = this.pending
            this.pending = null
            if (next) this.save(next.value)
        }, this.windowMs)
    }

    dispose() {
        if (this.timer) clearTimeout(this.timer)
        this.timer = null
        this.pending = null
    }
}

/** Every theme token's current value, for the frame to apply (FR-022, FR-023). */
export function themeTokens(style: Pick<CSSStyleDeclaration, 'getPropertyValue'>): Record<string, string> {
    const tokens: Record<string, string> = {}
    for (const name of ROLE_VARS) {
        const value = style.getPropertyValue(name).trim()
        if (value) tokens[name] = value
    }
    return tokens
}

/** A message to the frame. */
export const toFrame = (type: string, body: Record<string, unknown> = {}) => ({ vz: PROTOCOL, type, ...body })
