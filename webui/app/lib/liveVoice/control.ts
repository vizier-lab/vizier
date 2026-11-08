/**
 * The one live voice client per open conversation, and the seam the socket hands frames to
 * (`specs/055-live-voice`).
 *
 * The socket belongs to the conversation store, and the controls belong to the composer; this
 * module is what connects them without either importing the other. One entry per (agent,
 * conversation), which is also what the server keys its registry on (FR-005).
 */

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

/** The person typed a message: the audio stops, the session stays live (FR-025). */
export function forwardLiveVoiceTyped(agentId: string, topicId: string): void {
    entries.get(key(agentId, topicId))?.client.onTyped()
}

/** For the tests and for a page that unmounts: drop every entry. */
export function clearLiveVoice(): void {
    entries.clear()
}
