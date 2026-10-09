import { create } from 'zustand'
import { getChatWebSocketUrl } from '../services/vizier'
import type {
  WebSocketJobFrame,
  WebSocketMessage,
  WebSocketReactionErrorFrame,
  WebSocketReactionMessage,
  WebSocketReactionsFrame,
  WebSocketResponse,
} from '../interfaces/types'
import { useBackgroundJobStore } from './backgroundJobStore'

interface ConnectionState {
  agentId: string | null
  topicId: string | null
  connected: boolean
  lastMessage: WebSocketResponse | null
  messageCount: number
  connect: (agentId: string, topicId: string) => void
  disconnect: () => void
  sendMessage: (msg: WebSocketMessage | WebSocketReactionMessage) => void
  clearLastMessage: () => void
}

let ws: WebSocket | null = null
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null

export type ReactionFrame = WebSocketReactionsFrame | WebSocketReactionErrorFrame

// Reaction frames are delivered to listeners rather than through `lastMessage`, like job
// frames go to their store: `lastMessage` keeps only the latest frame, and a reaction frame
// landing in the same tick as a response must not cost the chat that response.
const reactionListeners = new Set<(frame: ReactionFrame) => void>()

export function subscribeReactionFrames(listener: (frame: ReactionFrame) => void): () => void {
  reactionListeners.add(listener)
  return () => {
    reactionListeners.delete(listener)
  }
}

export const useConnectionStore = create<ConnectionState>()((set, get) => ({
  agentId: null,
  topicId: null,
  connected: false,
  lastMessage: null,
  messageCount: 0,

  connect: (agentId: string, topicId: string) => {
    const state = get()

    // Already connected to the same session
    if (state.agentId === agentId && state.topicId === topicId && ws?.readyState === WebSocket.OPEN) {
      return
    }

    // Tear down existing connection
    if (ws) {
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout)
        reconnectTimeout = null
      }
      ws.onmessage = null
      ws.onclose = null
      ws.onerror = null
      ws.close()
      ws = null
    }

    set({ agentId, topicId, connected: false, lastMessage: null, messageCount: 0 })

    const url = getChatWebSocketUrl(agentId, topicId)
    const token = localStorage.getItem('auth_token')
    if (!token) return

    const doConnect = () => {
      ws = new WebSocket(url)

      ws.onopen = () => {
        console.log('Connection store: WebSocket connected')
        set({ connected: true })
        // (Re)connected: whatever happened to background jobs while the socket was down is
        // re-read rather than replayed.
        void useBackgroundJobStore.getState().load(agentId, topicId)
      }

      ws.onclose = () => {
        console.log('Connection store: WebSocket disconnected')
        set({ connected: false })

        // Auto-reconnect if we still have the same agent/topic
        const current = get()
        if (current.agentId === agentId && current.topicId === topicId && localStorage.getItem('auth_token')) {
          reconnectTimeout = setTimeout(doConnect, 3000)
        }
      }

      ws.onerror = (e) => {
        console.error('Connection store: WebSocket error', e)
      }

      ws.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data) as WebSocketResponse | WebSocketJobFrame
          // A job frame goes to the job store and never to `lastMessage`, so the chat's
          // response handler cannot mistake it for a response.
          if (parsed && typeof parsed === 'object' && 'background_job' in parsed) {
            useBackgroundJobStore.getState().applySnapshot(parsed.background_job)
            return
          }
          if (parsed && typeof parsed === 'object' && ('reactions' in parsed || 'reaction_error' in parsed)) {
            const frame = parsed as unknown as ReactionFrame
            reactionListeners.forEach((listener) => listener(frame))
            return
          }
          const data = parsed as WebSocketResponse
          set(state => ({
            lastMessage: data,
            messageCount: state.messageCount + 1,
          }))
        } catch (err) {
          console.error('Connection store: Failed to parse message', err)
        }
      }
    }

    doConnect()
  },

  disconnect: () => {
    if (reconnectTimeout) {
      clearTimeout(reconnectTimeout)
      reconnectTimeout = null
    }
    if (ws) {
      ws.close()
      ws = null
    }
    set({ agentId: null, topicId: null, connected: false, lastMessage: null })
  },

  sendMessage: (msg: WebSocketMessage | WebSocketReactionMessage) => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg))
    } else {
      console.error('Connection store: WebSocket not open')
    }
  },

  clearLastMessage: () => {
    set({ lastMessage: null })
  },
}))
