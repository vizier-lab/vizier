// The actions of an interactive page inside a shared copy. The page keeps its own state and its
// `run` functions, so it still works as a page; `sendMessage` would post to a conversation that
// does not exist here, so it does nothing but say so (FR-026, TP-44).
import type { StateStore } from '@json-render/core'
import { createActionHandlers } from '../generativeUi/actions'

export const NOT_IN_A_COPY = 'Not available in a copy of a conversation'

export interface ViewerActionDeps {
    store: StateStore
    runLogic?: (fn: string, args: unknown[]) => Promise<unknown>
    /** Called whenever the page tried to talk to the agent. */
    onBlocked: () => void
    onError?: (fn: string, message: string) => void
    onOk?: (fn: string) => void
}

export function createViewerHandlers(deps: ViewerActionDeps) {
    const base = createActionHandlers({
        store: deps.store,
        runLogic: deps.runLogic,
        // Never reached: `sendMessage` is replaced below.
        sendEvent: () => undefined,
        isTrusted: () => false,
        onError: deps.onError,
        onOk: deps.onOk,
    })
    return {
        ...base,
        sendMessage: () => {
            deps.onBlocked()
        },
    }
}
