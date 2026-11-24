/** Where the page is: what the secure-context rule needs, whatever edition it is. */
export type PwaAddress = {
    /** `location.hostname`. */
    hostname: string
    /** `location.protocol`, e.g. `https:` or `http:`. */
    protocol: string
    /** True in the desktop app's window. The secure-context rule does not need it. */
    desktop?: boolean
}

/** Where the app is running, as far as the worker cares. */
export type PwaOrigin = PwaAddress & {
    /** True in the desktop app's window. */
    desktop: boolean
}

/** True on a secure origin: HTTPS, or loopback over plain HTTP. */
export function isSecureOrigin({ hostname, protocol }: PwaAddress): boolean {
    if (protocol === 'https:') return true
    return hostname === '127.0.0.1' || hostname === 'localhost' || hostname === '[::1]'
}

/** True on a plain-HTTP address that is not loopback: the LAN case, where the browser refuses the worker. */
export function isInsecureNetwork({ hostname, protocol }: PwaAddress): boolean {
    return protocol === 'http:' && !isSecureOrigin({ hostname, protocol })
}

/**
 * Whether to register the service worker.
 *
 * The desktop app has its own updater and must never register one. Plain HTTP on a LAN
 * address is not a secure context, so the browser refuses the worker anyway; the app
 * says so rather than trying and failing.
 */
export function shouldRegister({ hostname, protocol, desktop }: PwaOrigin): boolean {
    if (desktop) return false
    return isSecureOrigin({ hostname, protocol })
}
