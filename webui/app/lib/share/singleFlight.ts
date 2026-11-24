// FR-050: an action that must not run twice at once. A second call while the first is running gets the
// first call's promise, so a double click or a key repeat does one thing. Once it settles, the next call runs again.
export function singleFlight<T>(): (run: () => Promise<T>) => Promise<T> {
    let current: Promise<T> | null = null
    return (run) => {
        if (current) return current
        current = run().finally(() => {
            current = null
        })
        return current
    }
}
