/**
 * The new worker is offered, not installed: nothing reloads until the person selects it.
 *
 * The toast is the same shape the desktop app's updater uses (root.tsx), so an update
 * reads the same way in both editions.
 */
export function watchForUpdate(onUpdate: (apply: () => void) => void): () => void {
    if (!('serviceWorker' in navigator)) return () => {}

    const announce = (worker: ServiceWorker) => {
        onUpdate(() => {
            worker.postMessage({ type: 'SKIP_WAITING' })
            // The new worker takes over and the app reloads into it.
            navigator.serviceWorker.addEventListener('controllerchange', () => location.reload(), { once: true })
        })
    }

    void navigator.serviceWorker.ready.then((registration) => {
        if (registration.waiting) announce(registration.waiting)
        registration.addEventListener('updatefound', () => {
            const installing = registration.installing
            if (!installing) return
            installing.addEventListener('statechange', () => {
                // A controller already exists, so this is a new worker rather than the first one.
                if (installing.state === 'installed' && navigator.serviceWorker.controller) announce(installing)
            })
        })
    })

    return () => {}
}
