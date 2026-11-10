import type { ConnectionState } from '../../api/transport/common'

/**
 * Whether the app should say it cannot reach the engine.
 *
 * `navigator.onLine` only says what the operating system thinks, and a LAN with no uplink still
 * reports "online". The topic sockets know more: a topic that lost its connection cannot reach
 * the engine, whatever the browser says.
 */
export function isUnreachable(offline: boolean, topicStates: Record<string, ConnectionState>): boolean {
    return offline || Object.values(topicStates).some((state) => state === 'offline')
}
