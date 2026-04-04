import { useEffect, useState, useRef, FormEvent } from 'react'
import { useParams, useNavigate } from 'react-router'
import useWebSocket, { ReadyState } from 'react-use-websocket'
import { getTopicHistory, getChatWebSocketUrl, listTopics, getAgentDetail } from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import type { Agent, ChatMessage, WebSocketMessage, WebSocketResponse } from '../interfaces/types'
import ReactMarkdown from 'react-markdown'
import { getCurrentUsername } from '../utils/auth'
import { Skeleton, SkeletonMessage } from '../components/Skeleton'

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
        setMessages(historyMessages)
      } catch (error) {
        console.error('Failed to load chat history:', error)
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

    const wsResponse = lastJsonMessage as any

    // Handle string format
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
      return
    }

    if ('ThinkingProgress' in wsResponse) {
      setIsThinking(true)
      setThinkingProgress(prev => prev + 1)
      return
    }

    if (wsResponse.Thinking) {
      setIsThinking(true)
      setCurrentThinking(`Using ${wsResponse.Thinking.name}...`)
      return
    }

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

    if ('Empty' in wsResponse) {
      setIsThinking(false)
      setCurrentThinking(null)
      setThinkingProgress(0)
      return
    }

    if ('Abort' in wsResponse) {
      setIsThinking(false)
      setCurrentThinking(null)
      setThinkingProgress(0)
      return
    }
  }, [lastJsonMessage, agentId, topicId])

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleCreateTopic = async () => {
    if (!newTopicId.trim() || !agentId) return

    const finalTopicId = autoCorrectSlugStrict(newTopicId)
    if (!finalTopicId) return

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
      <>
        <div className="main-header">
          <Skeleton variant="text" width={200} height={24} />
        </div>
        <div className="main-body" style={{ display: 'flex', flexDirection: 'column', gap: '2rem' }}>
          <SkeletonMessage />
          <SkeletonMessage />
          <SkeletonMessage />
        </div>
      </>
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
              <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px', fontFamily: 'var(--font-mono)' }}>
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
            <span style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              background: readyState === ReadyState.OPEN ? 'var(--accent-primary)' : 'var(--text-tertiary)',
              display: 'inline-block',
            }} />
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
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
        }}>
          <span style={{
            width: '6px',
            height: '6px',
            borderRadius: '50%',
            background: readyState === ReadyState.OPEN ? 'var(--accent-primary)' : '#f59e0b',
            display: 'inline-block',
          }} />
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
            flexDirection: 'column',
            gap: '1rem',
          }}>
            <div style={{ fontSize: '48px', opacity: 0.5 }}>💬</div>
            <p>No messages yet. Start the conversation!</p>
          </div>
        ) : (
          <>
            {messages.map((msg) => {
              const isUserMessage = msg.content.Request !== undefined
              let content: string | undefined
              let senderName: string = 'Unknown'

              if (isUserMessage && msg.content.Request) {
                const request = msg.content.Request as any
                if (request.content?.Chat) {
                  content = request.content.Chat
                } else if (typeof request.content === 'string') {
                  content = request.content
                }
                senderName = request.user || 'You'
              } else if (!isUserMessage && msg.content.Response) {
                const response = msg.content.Response as any
                if (Array.isArray(response)) {
                  content = response[0]
                } else if (typeof response === 'object' && 'content' in response) {
                  content = response.content
                } else if (typeof response === 'string') {
                  content = response
                }
                senderName = agentDetail?.name || 'Agent'
              }

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
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px',
                  }}>
                    <div style={{
                      fontWeight: '600',
                      fontSize: '14px',
                      color: isUserMessage ? 'var(--text-primary)' : 'var(--accent-primary)',
                    }}>
                      {senderName}
                    </div>
                  </div>
                  <div className="prose" style={{
                    padding: '12px 16px',
                    background: isUserMessage ? 'var(--surface)' : 'transparent',
                    borderRadius: '8px',
                    borderLeft: isUserMessage ? 'none' : '3px solid var(--accent-primary)',
                    boxShadow: isUserMessage ? 'var(--shadow-sm)' : 'none',
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
                  color: 'var(--accent-primary)',
                }}>
                  {agentDetail?.name || 'Agent'}
                </div>
                <div style={{
                  padding: '12px 16px',
                  borderRadius: '8px',
                  borderLeft: '3px solid var(--accent-primary)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  color: 'var(--text-tertiary)',
                  fontStyle: 'italic',
                  background: 'var(--surface)',
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
      }}>
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
