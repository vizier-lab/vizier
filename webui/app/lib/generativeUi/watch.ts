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

/**
 * Watch `watches` on `subscribe` (which fires on local changes only). A changed path runs its
 * bindings in order, untrusted, at most `MAX_CHAIN` of them; changes made while a chain runs,
 * by that chain, do not start another. Returns the stop function.
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

    const refresh = () => {
        for (const w of watches) seen.set(w.path, store.get(w.path))
    }

    const unsubscribe = subscribe(() => {
        if (running || stopped) return
        const changed = watches.filter((w) => store.get(w.path) !== seen.get(w.path))
        if (changed.length === 0) return
        refresh()
        running = true
        void (async () => {
            try {
                for (const w of changed) {
                    for (const binding of w.bindings.slice(0, MAX_CHAIN)) {
                        if (stopped) return
                        noteGesture(false)
                        const params = Object.fromEntries(
                            Object.entries(binding.params ?? {}).map(([key, value]) => [key, resolveActionParam(value, { stateModel: store.getSnapshot() })])
                        )
                        await execute({ ...binding, params })
                    }
                }
            } catch {
                // A failed action stops the chain; it has reported itself.
            } finally {
                // What the chain wrote is its own doing, not a change to react to.
                refresh()
                running = false
            }
        })()
    })

    return () => {
        stopped = true
        unsubscribe()
    }
}
