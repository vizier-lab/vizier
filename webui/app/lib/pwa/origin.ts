import type { PwaAddress } from './guard'

/** Where this page is, as the browser reports it. */
export function pageOrigin(): PwaAddress {
    return { hostname: location.hostname, protocol: location.protocol }
}
