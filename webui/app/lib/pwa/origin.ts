import { useMemo } from 'react'
import { isInsecureNetwork, type PwaAddress } from './guard'

/** Where this page is, as the browser reports it. */
export function pageOrigin(): PwaAddress {
    return { hostname: location.hostname, protocol: location.protocol }
}

/**
 * Whether this page is on a plain-HTTP address that is not loopback: the browser refuses the
 * worker there, so the app says so instead of trying and failing.
 */
export function useInsecureNetwork(): boolean {
    return useMemo(() => isInsecureNetwork(pageOrigin()), [])
}
