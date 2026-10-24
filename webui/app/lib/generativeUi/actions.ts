// The two actions json-render does not provide (`sendMessage`, `run`), as handlers. The rest
// (`setState`, `pushState`, `removeState`, `validateForm`) are json-render's own.
//
// `sendMessage` posts as the person, so it is held to three rules enforced here, out of the
// interface's reach (research R9, data-model §4): the chain must have started at the person's
// own click, key or tap; at most one a second; and bounded text and data.
import type { StateStore } from '@json-render/core'
import { LIMITS } from './limits'

export type ActionDeps = {
    store: StateStore
    /** Start an agent turn from this interface (the client event). */
    sendEvent: (text: string, data?: unknown) => void
    /** Whether the running chain began at a person's gesture. */
    isTrusted: () => boolean
    now?: () => number
    /** Call a function from the interface's `logic`. Absent: `run` refuses. */
    runLogic?: (fn: string, args: unknown[]) => Promise<unknown>
    /** After a message was sent, for the short "sent" notice. */
    onSent?: () => void
    /** A failure to show under the interface. */
    onError?: (fn: string, message: string) => void
    /** After a function ran well, to clear its last error. */
    onOk?: (fn: string) => void
}

type Params = Record<string, unknown>

export class ActionRefused extends Error {}

const byteLength = (value: unknown) => new TextEncoder().encode(JSON.stringify(value) ?? '').length

export function createActionHandlers(deps: ActionDeps) {
    const now = deps.now ?? Date.now
    let lastSend = -Infinity

    return {
        sendMessage: (params: Params) => {
            if (!deps.isTrusted()) throw new ActionRefused('sendMessage only works after the person clicks or types in the interface')
            const t = now()
            if (t - lastSend < LIMITS.sendMessageIntervalMs) throw new ActionRefused('sendMessage is limited to one a second')
            const text = params.text
            if (typeof text !== 'string' || text.trim() === '') throw new ActionRefused('sendMessage needs text')
            if (text.length > LIMITS.sendMessageText) throw new ActionRefused(`sendMessage text is over ${LIMITS.sendMessageText} characters`)
            if (params.data !== undefined && byteLength(params.data) > LIMITS.sendMessageDataBytes) {
                throw new ActionRefused(`sendMessage data is over ${LIMITS.sendMessageDataBytes / 1024} KB`)
            }
            lastSend = t
            deps.sendEvent(text, params.data)
            deps.onSent?.()
        },

        run: async (params: Params) => {
            const fn = typeof params.fn === 'string' ? params.fn : ''
            const args = Array.isArray(params.args) ? params.args : []
            if (!deps.runLogic) throw new ActionRefused('this interface has no logic to run')
            try {
                const result = await deps.runLogic(fn, args)
                if (typeof params.into === 'string' && params.into) deps.store.set(params.into, result)
                deps.onOk?.(fn)
            } catch (err) {
                // Nothing was written; the chain stops, and the person sees which function failed.
                deps.onError?.(fn, err instanceof Error ? err.message : String(err))
                throw err
            }
        },
    }
}
