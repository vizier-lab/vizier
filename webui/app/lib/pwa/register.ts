import { isDesktop } from '../../api/transport'
import { shouldRegister } from './guard'

/**
 * Registers the service worker.
 *
 * The desktop app has its own updater and must never register one. Plain HTTP on a LAN
 * address is not a secure context, so the browser refuses the worker anyway; the app
 * says so rather than trying and failing.
 */
export function registerServiceWorker(): void {
    if (!('serviceWorker' in navigator)) return
    const origin = { hostname: location.hostname, protocol: location.protocol, desktop: isDesktop }
    if (!shouldRegister(origin)) return

    void navigator.serviceWorker.register('/service-worker.js', { scope: '/' }).catch(() => {
        /* registration failed: the app works without it */
    })
}
