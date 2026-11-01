import { create } from 'zustand'
import { CHANNEL } from '../api/endpoints/chat'
import { openTopic, type ConnectionState, type TopicHandle } from '../api/socket'
import { withClientExtras } from '../lib/clientContext'

/** The key of a topic's state: agent and topic (the WebUI has one channel). */
export const topicKey = (agentId: string, topicId: string) => `${agentId}/${topicId}`

type Handlers = {
    onFrame: (data: unknown) => void
    onOpen: (reopened: boolean) => void
}

type ConnectionStore = {
    state: Record<string, ConnectionState>
    /** Opens the socket for a topic, or does nothing if it is already open. */
    open: (agentId: string, topicId: string, handlers: Handlers, channel?: string) => void
    close: (agentId: string, topicId: string) => void
    /** Sends a frame, adding this client's declarations and what it renders to a request. */
    send: (agentId: string, topicId: string, frame: object) => boolean
    retry: (agentId: string, topicId: string) => void
}

const sockets = new Map<string, TopicHandle>()

export const useConnection = create<ConnectionStore>()((set) => ({
    state: {},

    open: (agentId, topicId, handlers, channel = CHANNEL) => {
        const key = topicKey(agentId, topicId)
        if (sockets.has(key)) return
        const setState = (s: ConnectionState) => set((cur) => ({ state: { ...cur.state, [key]: s } }))
        sockets.set(
            key,
            openTopic(
                { agentId, channelId: channel, topicId },
                { onFrame: handlers.onFrame, onOpen: handlers.onOpen, onState: setState }
            )
        )
    },

    close: (agentId, topicId) => {
        const key = topicKey(agentId, topicId)
        sockets.get(key)?.close()
        sockets.delete(key)
        set((cur) => {
            const { [key]: _gone, ...rest } = cur.state
            return { state: rest }
        })
    },

    send: (agentId, topicId, frame) =>
        sockets.get(topicKey(agentId, topicId))?.send(withClientExtras(frame)) ?? false,

    retry: (agentId, topicId) => sockets.get(topicKey(agentId, topicId))?.retryNow(),
}))
