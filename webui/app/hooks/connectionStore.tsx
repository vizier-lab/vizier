import { create } from 'zustand'
import { getChatWebSocketUrl } from '../services/vizier'
import type { WebSocketMessage, WebSocketResponse } from '../interfaces/types'

interface ConnectionState {
  agentId: string | null
  topicId: string | null
  connected: boolean
  lastMessage: WebSocketResponse | null
  messageCount: number
  connect: (agentId: string, topicId: string) => void
  disconnect: () => void
  sendMessage: (msg: WebSocketMessage) => void
  clearLastMessage: () => void
}

let ws: WebSocket | null = null
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null

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

    // Close existing connection
    if (ws) {
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout)
        reconnectTimeout = null
      }
      ws.close()
      ws = null
    }

    set({ agentId, topicId, connected: false })

    const url = getChatWebSocketUrl(agentId, topicId)
    const token = localStorage.getItem('auth_token')
    if (!token) return

    const doConnect = () => {
      ws = new WebSocket(url)

      ws.onopen = () => {
        console.log('Connection store: WebSocket connected')
        set({ connected: true })
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
          const data = JSON.parse(event.data) as WebSocketResponse
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

  sendMessage: (msg: WebSocketMessage) => {
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
