/**
 * The one live voice client per open conversation, and the seam the socket hands frames to
 * (`specs/055-live-voice`).
 *
 * The socket belongs to the conversation store, and the controls belong to the composer; this
 * module is what connects them without either importing the other. One entry per (agent,
 * conversation), which is also what the server keys its registry on (FR-005).
 */

import { topicKey, useConnection } from '../../stores/connection'
import type { LiveVoiceClient, LiveVoiceView } from './session'

const key = (agentId: string, topicId: string) => `${agentId}/${topicId}`

interface Entry {
    client: LiveVoiceClient
    render: (view: LiveVoiceView) => void
}

const entries = new Map<string, Entry>()

/** Register the client the composer built, so incoming frames reach it. */
export function bindLiveVoice(
    agentId: string,
    topicId: string,
    client: LiveVoiceClient,
    render: (view: LiveVoiceView) => void
): () => void {
    entries.set(key(agentId, topicId), { client, render })
    return () => {
        const current = entries.get(key(agentId, topicId))
        if (current?.client === client) entries.delete(key(agentId, topicId))
    }
}

/**
 * Leaving by any means — an unmount, a hidden page, a closed tab — ends the session through here
 * (T090, FR-028/FR-045). Two jobs, in this order: the client lets go of the microphone and the
 * audio context through its own one release path, and the entry goes, so a frame that was already
 * on the wire has nowhere to land.
 *
 * Safe to call twice: the client's `stop()` is idempotent, and the second call finds no entry. A
 * newer client bound to the same conversation is left alone — the old surface leaving is not the
 * new one leaving.
 */
export function endLiveVoice(
    agentId: string,
    topicId: string,
    client: LiveVoiceClient | null,
    reason: 'pressed_again' | 'left_conversation' = 'left_conversation'
): void {
    client?.stop(reason)
    const current = entries.get(key(agentId, topicId))
    if (!current || current.client === client) entries.delete(key(agentId, topicId))
}

/**
 * Hand one socket frame to this conversation's client, when it knows the frame. Returns whether it
 * was a live voice frame, so the caller can stop classifying it.
 */
export function forwardLiveVoiceFrame(agentId: string, topicId: string, raw: unknown): boolean {
    if (!raw || typeof raw !== 'object' || !('live_voice' in raw)) return false
    const entry = entries.get(key(agentId, topicId))
    entry?.client.onFrame(raw)
    return true
}

/** The socket dropped or came back: the session must say so rather than look live (FR-033). */
export function forwardLiveVoiceState(
    agentId: string,
    topicId: string,
    state: 'open' | 'connecting' | 'reconnecting' | 'offline'
): void {
    entries.get(key(agentId, topicId))?.client.onSocketState(state)
}

/**
 * Hand the socket's state to this conversation's call, for as long as the caller keeps the
 * subscription (FR-033).
 *
 * `forwardLiveVoiceState` existed and nobody called it: the transport does publish
 * `reconnecting`/`offline`/`open` into the connection store, but the call never listened, so
 * `onSocketState` never ran in production. The consequence was not cosmetic — `droppedAt` was
 * never set, the thirty second deadline never armed, no `live_voice_start` was re-sent on a
 * return, and a session the server had already forgotten stayed on screen as `listening`
 * forever while every audio frame it sent was dropped in silence.
 *
 * Subscribing rather than reading the state in a render keeps the call out of React's render
 * path: a socket that flaps ten times a second should not re-render a bar. The current value is
 * forwarded on subscribe, because a session that starts while the socket is already open needs to
 * know that it is open.
 */
export function followLiveVoiceConnection(agentId: string, topicId: string): () => void {
    const id = key(agentId, topicId)
    const at = (state: Record<string, string> | undefined) => state?.[id]
    let seen = at(useConnection.getState().state)
    if (seen) forwardLiveVoiceState(agentId, topicId, seen as 'open' | 'connecting' | 'reconnecting' | 'offline')
    return useConnection.subscribe((current) => {
        const next = at(current.state)
        if (next === seen) return
        seen = next
        if (next) {
            forwardLiveVoiceState(agentId, topicId, next as 'open' | 'connecting' | 'reconnecting' | 'offline')
        }
    })
}

/** The person typed a message: the audio stops, the session stays live (FR-025). */
export function forwardLiveVoiceTyped(agentId: string, topicId: string): void {
    entries.get(key(agentId, topicId))?.client.onTyped()
}

/** For the tests and for a page that unmounts: drop every entry. */
export function clearLiveVoice(): void {
    entries.clear()
}
