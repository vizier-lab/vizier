// One live connection per open topic. The connection itself belongs to the transport
// (a WebSocket over HTTP, a Tauri channel in the desktop app); this module keeps the names callers use.
import { transport, type ConnectionState, type TopicHandle, type TopicOptions } from './transport'

export type { ConnectionState, TopicHandle, TopicOptions }
export { backoffDelay, stateAfterFailures, OFFLINE_AFTER } from './transport/websocket'

export const openTopic = (t: { agentId: string; channelId: string; topicId: string }, opts: TopicOptions): TopicHandle => transport.openTopic(t, opts)
