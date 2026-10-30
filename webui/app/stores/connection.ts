import { create } from 'zustand'
import { CHANNEL } from '../api/endpoints/chat'
import { openTopic, type ConnectionState, type TopicHandle } from '../api/socket'
import { declaredClientTools } from '../lib/clientTools'

/** The key of a topic's state: agent and topic (the WebUI has one channel). */
export const topicKey = (agentId: string, topicId: string) => `${agentId}/${topicId}`

type Handlers = {
    onFrame: (data: unknown) => void
    onOpen: (reopened: boolean) => void
}

type ConnectionStore = {
    state: Record<string, ConnectionState>
    /** Opens the socket for a topic, or does nothing if it is already open. */
    open: (agentId: string, topicId: string, handlers: Handlers) => void
    close: (agentId: string, topicId: string) => void
    /** Sends a frame, adding this client's tool declarations to a request. */
    send: (agentId: string, topicId: string, frame: object) => boolean
    retry: (agentId: string, topicId: string) => void
}

const sockets = new Map<string, TopicHandle>()

/** A request carries the tools this client offers; commands, reactions and results don't. */
function withClientTools(frame: object): object {
    if (!('content' in frame) || (frame as { content: object }).content === null) return frame
    const content = (frame as { content: object }).content
    if ('command' in content) return frame
    const declared = declaredClientTools()
    return declared.client_tools.length === 0 ? frame : { ...frame, ...declared }
}

export const useConnection = create<ConnectionStore>()((set) => ({
    state: {},

    open: (agentId, topicId, handlers) => {
        const key = topicKey(agentId, topicId)
        if (sockets.has(key)) return
        const setState = (s: ConnectionState) => set((cur) => ({ state: { ...cur.state, [key]: s } }))
        sockets.set(
            key,
            openTopic(
                { agentId, channelId: CHANNEL, topicId },
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

    send: (agentId, topicId, frame) => sockets.get(topicKey(agentId, topicId))?.send(withClientTools(frame)) ?? false,

    retry: (agentId, topicId) => sockets.get(topicKey(agentId, topicId))?.retryNow(),
}))
