import { create } from 'zustand'
import { getChatWebSocketUrl } from '../services/vizier'
import type {
  ClientFrame,
  ClientToolCallFrame,
  WebSocketJobFrame,
  WebSocketMessage,
  WebSocketReactionErrorFrame,
  WebSocketReactionMessage,
  WebSocketReactionsFrame,
  WebSocketResponse,
} from '../interfaces/types'
import { useBackgroundJobStore } from './backgroundJobStore'
import { useToastStore } from './toastStore'
import { declaredClientTools, handleClientCall } from '../lib/clientTools'
import { getCurrentUsername } from '../utils/auth'

interface ConnectionState {
  agentId: string | null
  topicId: string | null
  connected: boolean
  lastMessage: WebSocketResponse | null
  messageCount: number
  connect: (agentId: string, topicId: string) => void
  disconnect: () => void
  sendMessage: (msg: WebSocketMessage | WebSocketReactionMessage) => void
  /** Start a turn from a client (protocol §5), e.g. a click in a page the agent drew. */
  sendClientEvent: (text: string, data?: unknown, sourceCallId?: string) => void
  /** Save a client tool call's state (protocol §6). */
  saveClientState: (callId: string, state: unknown) => void
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

// Client tool frames (specs/019-client-tools) go to listeners too, for the same reason.
const clientListeners = new Set<(frame: ClientFrame) => void>()

export function subscribeClientFrames(listener: (frame: ClientFrame) => void): () => void {
  clientListeners.add(listener)
  return () => {
    clientListeners.delete(listener)
  }
}

const CLIENT_FRAME_KEYS = [
  'client_tool_call',
  'client_tools_resolved',
  'client_tool_outcome',
  'client_call_state',
  'client_tool_cancelled',
  'client_error',
] as const

function isClientFrame(value: object): value is ClientFrame {
  return CLIENT_FRAME_KEYS.some((key) => key in value)
}

// Calls the server stopped waiting for. A `handle` still running for one has its answer
// dropped, since the server would only reply `not_pending`.
const cancelledCalls = new Set<string>()

/// Closes the current socket with every handler detached first, so a socket on its way out
/// can neither deliver a late frame into the next session nor flip `connected` or schedule a
/// reconnect over the socket that replaced it.
function teardown() {
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout)
    reconnectTimeout = null
  }
  if (ws) {
    ws.onopen = null
    ws.onmessage = null
    ws.onclose = null
    ws.onerror = null
    ws.close()
    ws = null
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

    // Already connected, or connecting, to the same session
    if (
      state.agentId === agentId &&
      state.topicId === topicId &&
      (ws?.readyState === WebSocket.OPEN || ws?.readyState === WebSocket.CONNECTING)
    ) {
      return
    }

    teardown()

    set({ agentId, topicId, connected: false, lastMessage: null, messageCount: 0 })

    const url = getChatWebSocketUrl(agentId, topicId)
    const token = localStorage.getItem('auth_token')
    if (!token) return

    const doConnect = () => {
      reconnectTimeout = null
      const socket = new WebSocket(url)
      ws = socket

      socket.onopen = () => {
        if (ws !== socket) return
        console.log('Connection store: WebSocket connected')
        set({ connected: true })
        // (Re)connected: whatever happened to background jobs while the socket was down is
        // re-read rather than replayed.
        void useBackgroundJobStore.getState().load(agentId, topicId)
      }

      socket.onclose = () => {
        // Only the live socket speaks for the connection; one already replaced has nothing
        // to say about it.
        if (ws !== socket) return
        console.log('Connection store: WebSocket disconnected')
        ws = null
        set({ connected: false })

        // Auto-reconnect if we still have the same agent/topic
        const current = get()
        if (current.agentId === agentId && current.topicId === topicId && localStorage.getItem('auth_token')) {
          reconnectTimeout = setTimeout(doConnect, 3000)
        }
      }

      socket.onerror = (e) => {
        console.error('Connection store: WebSocket error', e)
      }

      socket.onmessage = (event) => {
        if (ws !== socket) return
        try {
          const parsed = JSON.parse(event.data) as WebSocketResponse | WebSocketJobFrame
          // A job frame goes to the job store and never to `lastMessage`, so the chat's
          // response handler cannot mistake it for a response.
          if (parsed && typeof parsed === 'object' && 'background_job' in parsed) {
            useBackgroundJobStore.getState().applySnapshot(parsed.background_job)
            return
          }
          if (parsed && typeof parsed === 'object' && isClientFrame(parsed)) {
            handleClientFrame(parsed, socket, agentId, topicId)
            clientListeners.forEach((listener) => listener(parsed))
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
    teardown()
    set({ agentId: null, topicId: null, connected: false, lastMessage: null })
  },

  sendMessage: (msg: WebSocketMessage | WebSocketReactionMessage) => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(withClientTools(msg)))
    } else {
      console.error('Connection store: WebSocket not open')
    }
  },

  sendClientEvent: (text: string, data?: unknown, sourceCallId?: string) => {
    get().sendMessage(clientEventMessage(text, data, sourceCallId))
  },

  saveClientState: (callId: string, state: unknown) => {
    sendRaw({ client_call_state: { call_id: callId, state } })
  },

  clearLastMessage: () => {
    set({ lastMessage: null })
  },
}))

/** A `client_event` request (protocol §5), as `sendClientEvent` sends it. */
export function clientEventMessage(text: string, data?: unknown, sourceCallId?: string): WebSocketMessage {
  return {
    timestamp: new Date().toISOString(),
    user: getCurrentUsername(),
    content: {
      client_event: {
        text,
        ...(data === undefined ? {} : { data }),
        ...(sourceCallId ? { source_call_id: sourceCallId } : {}),
      },
    },
    metadata: {},
  }
}

/** Every request declares the registered client tools; reactions and commands declare none. */
function withClientTools(msg: WebSocketMessage | WebSocketReactionMessage): object {
  if (!('content' in msg) || 'command' in msg.content) return msg
  const declared = declaredClientTools()
  if (declared.client_tools.length === 0) return msg
  return { ...msg, ...declared }
}

function sendRaw(frame: object): boolean {
  if (ws?.readyState !== WebSocket.OPEN) {
    console.error('Connection store: WebSocket not open')
    return false
  }
  ws.send(JSON.stringify(frame))
  return true
}

/** The connection's own part in a client frame: answering calls, tracking cancellations. */
function handleClientFrame(frame: ClientFrame, socket: WebSocket, agentId: string, topicId: string) {
  if ('client_tool_cancelled' in frame) {
    cancelledCalls.add(frame.client_tool_cancelled.call_id)
    return
  }
  if ('client_error' in frame) {
    const { code, message, tool, rule } = frame.client_error
    console.warn('Client tools:', code, tool ?? '', rule ?? '', message)
    if (code === 'invalid_client_tools') {
      useToastStore
        .getState()
        .addToast('error', 'A client tool was refused', `${tool ?? ''} ${message}`.trim())
    }
    return
  }
  if ('client_tool_call' in frame && frame.client_tool_call.answer) {
    void answerClientCall(frame.client_tool_call, socket, agentId, topicId)
  }
}

async function answerClientCall(
  call: ClientToolCallFrame['client_tool_call'],
  socket: WebSocket,
  agentId: string,
  topicId: string
) {
  const store = useConnectionStore.getState()
  const answer = await handleClientCall(call, {
    callId: call.call_id,
    agentId,
    topicId,
    sendEvent: (text, data) => store.sendClientEvent(text, data, call.call_id),
    saveState: (state) => store.saveClientState(call.call_id, state),
  })
  if (cancelledCalls.delete(call.call_id)) return
  // Only the socket that was asked may answer; a replaced one has nothing pending.
  if (ws !== socket || socket.readyState !== WebSocket.OPEN) return
  socket.send(JSON.stringify({ client_tool_result: { call_id: call.call_id, ...answer } }))
}
