// `watch` for a drawn interface: run bindings when a state path changes. json-render has its own,
// but it cannot tell a change made here from one merged in from another tab, and a watch chain
// must not be trusted or loop on its own writes (contracts/ui-catalogue.md §2), so it is ours.
import { resolveActionParam, type StateStore } from '@json-render/core'
import { noteGesture } from './trust'

export const MAX_CHAIN = 8

export type WatchBinding = { action: string; params?: Record<string, unknown> }
export type Watches = { path: string; bindings: WatchBinding[] }[]

/** Every `watch` of the spec's elements. */
export function collectWatches(spec: { elements: Record<string, { watch?: Record<string, unknown> }> }): Watches {
    const out: Watches = []
    for (const element of Object.values(spec.elements)) {
        for (const [path, binding] of Object.entries(element.watch ?? {})) {
            const list = (Array.isArray(binding) ? binding : [binding]).filter((b): b is WatchBinding => !!b && typeof (b as WatchBinding).action === 'string')
            if (list.length) out.push({ path, bindings: list })
        }
    }
    return out
}

/** The state paths a binding writes, from its literal `into` and `statePath` params. */
function writtenBy(binding: WatchBinding): string[] {
    return ['into', 'statePath'].flatMap((key) => (typeof binding.params?.[key] === 'string' ? [binding.params[key] as string] : []))
}

const MAX_ROUNDS = 50

/**
 * Watch `watches` on `subscribe` (which fires on local changes only). A changed path runs its
 * bindings in order, untrusted, at most `MAX_CHAIN` of them. A chain's own writes (its `into` and
 * `statePath`) do not start another chain, but a change made by the person while a chain runs
 * does: it is picked up when the chain ends, so a dragged slider always ends on its last value.
 * Returns the stop function.
 */
export function startWatching(
    watches: Watches,
    store: StateStore,
    subscribe: (listener: () => void) => () => void,
    execute: (binding: WatchBinding) => Promise<void>
): () => void {
    if (watches.length === 0) return () => {}
    const seen = new Map(watches.map((w) => [w.path, store.get(w.path)]))
    let running = false
    let stopped = false

    const drain = async () => {
        running = true
        try {
            for (let round = 0; round < MAX_ROUNDS && !stopped; round++) {
                const changed = watches.filter((w) => store.get(w.path) !== seen.get(w.path))
                if (changed.length === 0) return
                for (const w of watches) seen.set(w.path, store.get(w.path))
                const written = new Set<string>()
                try {
                    for (const w of changed) {
                        for (const binding of w.bindings.slice(0, MAX_CHAIN)) {
                            if (stopped) return
                            noteGesture(false)
                            const params = Object.fromEntries(
                                Object.entries(binding.params ?? {}).map(([key, value]) => [key, resolveActionParam(value, { stateModel: store.getSnapshot() })])
                            )
                            writtenBy(binding).forEach((path) => written.add(path))
                            await execute({ ...binding, params })
                        }
                    }
                } catch {
                    // A failed action stops its chain; it has reported itself.
                }
                // What the chain wrote is its own doing, not a change to react to.
                for (const path of written) if (seen.has(path)) seen.set(path, store.get(path))
            }
        } finally {
            running = false
        }
    }

    const unsubscribe = subscribe(() => {
        if (running || stopped) return
        void drain()
    })

    return () => {
        stopped = true
        unsubscribe()
    }
}
