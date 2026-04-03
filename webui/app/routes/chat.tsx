import { useEffect, useState, useRef, FormEvent } from 'react'
import { useParams, useNavigate } from 'react-router'
import useWebSocket, { ReadyState } from 'react-use-websocket'
import { getTopicHistory, getChatWebSocketUrl, listTopics, getAgentDetail } from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import type { Agent, ChatMessage, WebSocketMessage, WebSocketResponse } from '../interfaces/types'
import ReactMarkdown from 'react-markdown'
import { getCurrentUsername } from '../utils/auth'

export default function Chat() {
  const { agentId, topicId } = useParams()
  const navigate = useNavigate()

  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(true)
  const [isNewTopic, setIsNewTopic] = useState(false)
  const [newTopicId, setNewTopicId] = useState('')
  const [showNewTopicInput, setShowNewTopicInput] = useState(false)
  const [isThinking, setIsThinking] = useState(false)
  const [currentThinking, setCurrentThinking] = useState<string | null>(null)
  const [thinkingProgress, setThinkingProgress] = useState(0)
  const [agentDetail, setAgentDetail] = useState<Agent | null>(null)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)

  // WebSocket connection
  const { sendJsonMessage, lastJsonMessage, readyState } = useWebSocket(
    agentId && topicId && topicId !== 'new'
      ? getChatWebSocketUrl(agentId, topicId)
      : null,
    {
      shouldReconnect: () => true,
      reconnectInterval: 3000,
    }
  )

  useEffect(() => {
    console.log('>>', { agentId })

  }, [agentId])

  // Check if this is a new topic
  useEffect(() => {
    if (topicId === 'new') {
      setIsNewTopic(true)
      setShowNewTopicInput(true)
      setLoading(false)
    } else {
      setIsNewTopic(false)
    }
  }, [topicId])

  // Load chat history
  useEffect(() => {
    if (!agentId || !topicId || topicId === 'new') return

    getAgentDetail(agentId).then(data => {
      setAgentDetail(data.data)
    })

    const loadHistory = async () => {
      try {
        const response = await getTopicHistory(agentId, topicId)
        const historyMessages = response.data || []

        // Validate and log message structure for debugging
        if (historyMessages.length > 0) {
          console.log('[Chat History Loaded]', {
            count: historyMessages.length,
            fullResponse: response,
            firstMessage: JSON.stringify(historyMessages[0], null, 2),
            lastMessage: JSON.stringify(historyMessages[historyMessages.length - 1], null, 2),
          })

          // Log what the renderer sees
          historyMessages.forEach((msg, idx) => {
            const hasRequest = msg.content?.Request !== undefined
            const hasResponse = msg.content?.Response !== undefined
            console.log(`[Message ${idx}]`, {
              uid: msg.uid,
              hasRequest,
              hasResponse,
              contentKeys: Object.keys(msg.content || {}),
              fullMessage: JSON.stringify(msg, null, 2),
            })
          })
        }

        setMessages(historyMessages)
      } catch (error) {
        console.error('Failed to load chat history:', error)
        // Set empty messages array to avoid indefinite loading state
        setMessages([])
      } finally {
        setLoading(false)
      }
    }

    loadHistory()
  }, [agentId, topicId])

  // Handle incoming WebSocket messages
  useEffect(() => {
    if (!lastJsonMessage) return

    // Log the raw response for debugging
    console.log('[WebSocket Response]', lastJsonMessage)

    const wsResponse = lastJsonMessage as any

    // Handle string format (Serde serializes unit variants as strings)
    if (typeof wsResponse === 'string') {
      if (wsResponse === 'ThinkingProgress') {
        setIsThinking(true)
        setThinkingProgress(prev => prev + 1)
      } else if (wsResponse === 'Empty') {
        setIsThinking(false)
        setCurrentThinking(null)
        setThinkingProgress(0)
      } else if (wsResponse === 'Abort') {
        setIsThinking(false)
        setCurrentThinking(null)
        setThinkingProgress(0)
      }
      return
    }

    // Handle object format
    if (typeof wsResponse !== 'object' || wsResponse === null) {
      console.warn('Invalid WebSocket response:', wsResponse)
      return
    }

    // Check for ThinkingProgress (might be { ThinkingProgress: null } or just have the key)
    if ('ThinkingProgress' in wsResponse) {
      setIsThinking(true)
      setThinkingProgress(prev => prev + 1)
      return
    }

    // Check for Thinking with tool info
    if (wsResponse.Thinking) {
      setIsThinking(true)
      setCurrentThinking(`Using ${wsResponse.Thinking.name}...`)
      return
    }

    // Check for Message
    if (wsResponse.Message) {
      setIsThinking(false)
      setCurrentThinking(null)
      setThinkingProgress(0)

      const newMessage: ChatMessage = {
        uid: Date.now().toString(),
        vizier_session: {
          agent_id: agentId!,
          channel: 'vizier-webui',
          topic: topicId!,
        },
        content: {
          Response: {
            content: wsResponse.Message.content,
          },
        },
        timestamp: new Date().toISOString(),
      }

      setMessages(prev => [...prev, newMessage])
      return
    }

    // Check for Empty
    if ('Empty' in wsResponse) {
      setIsThinking(false)
      setCurrentThinking(null)
      setThinkingProgress(0)
      return
    }

    // Check for Abort
    if ('Abort' in wsResponse) {
      setIsThinking(false)
      setCurrentThinking(null)
      setThinkingProgress(0)
      return
    }

    console.warn('Unknown response type:', wsResponse)
  }, [lastJsonMessage, agentId, topicId])

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleCreateTopic = async () => {
    if (!newTopicId.trim() || !agentId) return

    // Apply strict validation to remove trailing hyphens before submitting
    const finalTopicId = autoCorrectSlugStrict(newTopicId)

    if (!finalTopicId) return // Ensure we have a valid slug

    // Navigate to the new topic
    navigate(`/${agentId}/chat/${finalTopicId}`)
    setShowNewTopicInput(false)
    setNewTopicId('')
  }

  const handleSendMessage = async (e: FormEvent) => {
    e.preventDefault()
    if (!input.trim() || !agentId || !topicId) return

    const username = getCurrentUsername()

    const message: WebSocketMessage = {
      user: username,
      content: { Chat: input.trim() },
      metadata: {},
    }

    // Add user message to display
    const userMessage: ChatMessage = {
      uid: Date.now().toString(),
      vizier_session: {
        agent_id: agentId,
        channel: 'vizier-webui',
        topic: topicId,
      },
      content: {
        Request: {
          user: username,
          content: input.trim(),
        },
      },
      timestamp: new Date().toISOString(),
    }

    setMessages(prev => [...prev, userMessage])
    setInput('')

    // Send via WebSocket
    sendJsonMessage(message)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSendMessage(e as any)
    }
  }

  if (loading) {
    return (
      <div className="main-body" style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}>
        Loading chat...
      </div>
    )
  }

  if (showNewTopicInput) {
    return (
      <div className="main-body" style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}>
        <div style={{
          maxWidth: '500px',
          width: '100%',
        }}>
          <h2 style={{ marginBottom: '1rem' }}>Create New Topic</h2>
          <p style={{
            color: 'var(--text-secondary)',
            marginBottom: '1.5rem',
            fontSize: '14px',
          }}>
            Enter a unique identifier for this conversation topic (e.g., "project-alpha", "daily-standup")
          </p>
          <div className="input-group">
            <label htmlFor="topic-id">Topic ID</label>
            <input
              id="topic-id"
              type="text"
              value={newTopicId}
              onChange={(e) => setNewTopicId(autoCorrectSlug(e.target.value))}
              placeholder="my-topic-id"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  handleCreateTopic()
                }
              }}
            />
            {newTopicId && (
              <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                Topic ID: {newTopicId}
              </div>
            )}
          </div>
          <div style={{
            display: 'flex',
            gap: '8px',
            marginTop: '1rem',
          }}>
            <button
              className="btn btn-primary"
              onClick={handleCreateTopic}
              disabled={!newTopicId.trim()}
            >
              Create Topic
            </button>
            <button
              className="btn btn-secondary"
              onClick={() => navigate('/')}
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    )
  }

  const connectionStatus = {
    [ReadyState.CONNECTING]: 'Connecting...',
    [ReadyState.OPEN]: 'Connected',
    [ReadyState.CLOSING]: 'Closing...',
    [ReadyState.CLOSED]: 'Disconnected',
    [ReadyState.UNINSTANTIATED]: 'Not connected',
  }[readyState]


  return (
    <>
      {/* Header */}
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0, display: 'flex', alignItems: 'center', gap: '12px' }}>
            # {topicId}
          </h3>
          <div style={{
            fontSize: '12px',
            color: 'var(--text-tertiary)',
            marginTop: '4px',
            fontFamily: 'var(--font-mono)',
          }}>
            Agent: {agentId}
          </div>
        </div>
        <div style={{
          fontSize: '12px',
          color: readyState === ReadyState.OPEN ? 'var(--text-tertiary)' : 'var(--text-secondary)',
        }}>
          {connectionStatus}
        </div>
      </div>

      {/* Messages */}
      <div
        className="main-body no-scrollbar"
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1.5rem',
        }}
      >
        {messages.length === 0 && !isThinking ? (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            color: 'var(--text-tertiary)',
          }}>
            No messages yet. Start the conversation!
          </div>
        ) : (
          <>
            {messages.map((msg) => {
              // Determine if this is a user message by checking for Request content
              const isUserMessage = msg.content.Request !== undefined

              // Extract content - handle both Request and Response formats
              let content: string | undefined
              let senderName: string = 'Unknown'

              if (isUserMessage && msg.content.Request) {
                // Request format: { user, content: { Chat: string }, metadata }
                const request = msg.content.Request as any
                if (request.content?.Chat) {
                  content = request.content.Chat
                } else if (typeof request.content === 'string') {
                  content = request.content
                }
                senderName = request.user || 'You'
              } else if (!isUserMessage && msg.content.Response) {
                // Response format: [string, stats?] (tuple format from backend)
                const response = msg.content.Response as any
                if (Array.isArray(response)) {
                  // Tuple: [content, stats]
                  content = response[0]
                } else if (typeof response === 'object' && 'content' in response) {
                  // Object format: { content, stats }
                  content = response.content
                } else if (typeof response === 'string') {
                  // Direct string
                  content = response
                }
                console.log('>>', { agentDetail })
                senderName = agentDetail?.name || ''
              }

              // Skip if content is empty
              if (!content) return null

              return (
                <div
                  key={msg.uid}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px',
                  }}
                >
                  <div style={{
                    fontWeight: '600',
                    fontSize: '14px',
                    color: isUserMessage ? 'var(--text-primary)' : 'var(--text-secondary)',
                  }}>
                    {senderName}
                  </div>
                  <div className="prose" style={{
                    padding: '12px 16px',
                    background: isUserMessage ? 'var(--surface)' : 'transparent',
                    borderRadius: '8px',
                    borderLeft: isUserMessage ? 'none' : '3px solid var(--border)',
                  }}>
                    <ReactMarkdown>{content}</ReactMarkdown>
                  </div>
                </div>
              )
            })}

            {/* Thinking indicator */}
            {isThinking && (
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '8px',
              }}>
                <div style={{
                  fontWeight: '600',
                  fontSize: '14px',
                  color: 'var(--text-secondary)',
                }}>
                  Agent
                </div>
                <div style={{
                  padding: '12px 16px',
                  borderRadius: '8px',
                  borderLeft: '3px solid var(--border)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  color: 'var(--text-tertiary)',
                  fontStyle: 'italic',
                }}>
                  <div className="thinking-dots">
                    <span>.</span>
                    <span>.</span>
                    <span>.</span>
                  </div>
                  {currentThinking || 'Thinking...'}
                </div>
              </div>
            )}
          </>
        )}
        <div ref={messagesEndRef} />
      </div >

      {/* Input */}
      < div style={{
        borderTop: '1px solid var(--border)',
        padding: '16px 24px',
        background: 'var(--background)',
      }
      }>
        <form onSubmit={handleSendMessage} style={{
          display: 'flex',
          gap: '12px',
          maxWidth: '900px',
          margin: '0 auto',
        }}>
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={readyState === ReadyState.OPEN ? "Type your message..." : "Connecting..."}
            disabled={readyState !== ReadyState.OPEN}
            rows={1}
            style={{
              flex: 1,
              resize: 'none',
              minHeight: '44px',
              maxHeight: '200px',
            }}
          />
          <button
            type="submit"
            className="btn btn-primary"
            disabled={!input.trim() || readyState !== ReadyState.OPEN}
          >
            Send
          </button>
        </form>
      </div >
    </>
  )
}
