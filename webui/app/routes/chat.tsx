import { useEffect, useState, useRef, FormEvent } from 'react'
import { useParams, useNavigate } from 'react-router'
import useWebSocket, { ReadyState } from 'react-use-websocket'
import { getTopicHistory, getChatWebSocketUrl, listTopics, getAgentDetail } from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import type { Agent, ChatMessage, Topic, WebSocketMessage, WebSocketResponse, VizierResponseStats } from '../interfaces/types'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import hljs from 'highlight.js'
import { getCurrentUsername } from '../utils/auth'
import { Skeleton, SkeletonMessage } from '../components/Skeleton'
import { FaPaperPlane } from 'react-icons/fa'
import { FiCopy, FiCheck } from 'react-icons/fi'
import { useToastStore } from '../hooks/toastStore'

const textareaStyle = `
  .chat-textarea::-webkit-scrollbar {
    display: none;
  }
  .chat-textarea {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
`

interface InlineEvent {
  id: string
  type: 'start' | 'tool_choice' | 'thinking'
  content?: string
  timestamp: number
}

const formatToolChoice = (name: string, args: Record<string, unknown>): string => {
  switch (name) {
    case 'think':
      return args.thought as string
    case 'memory_read':
      return `**search memory:** '${args.query as string}'`
    case 'memory_write':
      return `**write memory:** '${args.title as string}'`
    case 'python_interpreter':
      return "**programatic tool**: \n```python" + `\n${args.script as string}\n` + "```"
    default:
      let formattedArgs = "```js" + `\n${JSON.stringify(args, null, 2)}\n` + "```";
      return `*use* **${name}** \n${formattedArgs}`
  }
}

export default function Chat() {
  const { agentId, topicId } = useParams()
  const navigate = useNavigate()

  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(true)
  const [isNewTopic, setIsNewTopic] = useState(false)
  const [newTopicId, setNewTopicId] = useState('')
  const [showNewTopicInput, setShowNewTopicInput] = useState(false)
  const [inlineEvents, setInlineEvents] = useState<InlineEvent[]>([])
  const [agentDetail, setAgentDetail] = useState<Agent | null>(null)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const thinkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const { addToast } = useToastStore()

  // WebSocket connection
  const wsUrl = agentId && topicId && topicId !== 'new'
    ? getChatWebSocketUrl(agentId, topicId)
    : null
  console.log('WebSocket URL:', wsUrl)

  const { sendJsonMessage, lastJsonMessage, readyState } = useWebSocket(
    wsUrl,
    {
      shouldReconnect: () => true,
      reconnectInterval: 3000,
      onOpen: () => console.log('WebSocket connected'),
      onClose: () => console.log('WebSocket disconnected'),
      onError: (e) => console.error('WebSocket error:', e),
    }
  )

  const [topicDetail, setTopicDetail] = useState<any | null>(null)

  // Check if this is a new topic
  useEffect(() => {
    if (topicId === 'new') {
      setIsNewTopic(true)
      setShowNewTopicInput(true)
      setLoading(false)
    } else {
      setIsNewTopic(false)
      setShowNewTopicInput(false)


      if (agentId) {
        console.log('>> ?', { agentId })
        listTopics(agentId).then(topic => {
          let topicDetail = topic.data.find((item: any) => item.topic_id == topicId);

          setTopicDetail(topicDetail)
        })
      }
    }
  }, [topicId])

  // Clear inline events when topic changes
  useEffect(() => {
    setInlineEvents([])
    if (thinkingTimeoutRef.current) {
      clearTimeout(thinkingTimeoutRef.current)
      thinkingTimeoutRef.current = null
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

  const clearInlineEvents = () => {
    setInlineEvents([])
    if (thinkingTimeoutRef.current) {
      clearTimeout(thinkingTimeoutRef.current)
      thinkingTimeoutRef.current = null
    }
  }

  const startThinkingTimeout = () => {
    if (thinkingTimeoutRef.current) {
      clearTimeout(thinkingTimeoutRef.current)
    }
    thinkingTimeoutRef.current = setTimeout(() => {
      clearInlineEvents()
    }, 3600000)
  }

  const addInlineEvent = (type: InlineEvent['type'], content?: string) => {
    setInlineEvents(prev => [...prev, {
      id: Date.now().toString() + Math.random().toString(36).substr(2, 9),
      type,
      content,
      timestamp: Date.now(),
    }])
  }

  // Handle incoming WebSocket messages
  useEffect(() => {
    console.log('WebSocket message received:', lastJsonMessage)
    if (!lastJsonMessage) return

    const wsResponse = lastJsonMessage as WebSocketResponse

    if (typeof wsResponse !== 'object' || wsResponse === null) {
      return
    }

    const { timestamp, content } = wsResponse

    switch (content) {
      case 'thinking_start':
        setInlineEvents([{ id: Date.now().toString(), type: 'start', timestamp: Date.now() }])
        startThinkingTimeout()
        return

      case 'empty':
        clearInlineEvents()
        return

      case 'abort':
        clearInlineEvents()
        return
    }

    if (typeof content === 'object') {
      if ('thinking' in content) {
        addInlineEvent('thinking', content.thinking)
        return
      }

      if ('tool_choice' in content) {
        const toolContent = formatToolChoice(content.tool_choice.name, content.tool_choice.args)
        addInlineEvent('tool_choice', toolContent)
        return
      }

      if ('message' in content) {
        clearInlineEvents()
        setMessages(prev => {
          if (prev.some(m => m.content.Response?.timestamp === timestamp)) {
            return prev
          }
          const newMessage: ChatMessage = {
            uid: timestamp,
            vizier_session: {
              agent_id: agentId!,
              channel: 'vizier-webui',
              topic: topicId!,
            },
            content: {
              Response: {
                timestamp,
                content,
              },
            },
          }
          return [...prev, newMessage]
        })
        return
      }
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

    try {
      const response = await listTopics(agentId)
      const topics = response.data || []
      const exists = topics.some((t: any) => t.topic_id === finalTopicId)

      if (exists) {
        addToast('error', 'Topic already exists', `A topic with ID "${finalTopicId}" already exists.`)
        return
      }
    } catch (error) {
      console.error('Failed to check topic existence:', error)
    }

    navigate(`/${agentId}/chat/${finalTopicId}`)
    setShowNewTopicInput(false)
    setNewTopicId('')
  }

  const handleSendMessage = async (e: FormEvent) => {
    e.preventDefault()
    if (!input.trim() || !agentId || !topicId) return

    console.log('WebSocket readyState before check:', readyState)
    if (readyState !== 1) { // 1 = OPEN
      console.error('WebSocket not ready, state:', readyState)
      return
    }

    const username = getCurrentUsername()

    const message: WebSocketMessage = {
      timestamp: new Date().toISOString(),
      user: username,
      content: { chat: input.trim() },
      metadata: null,
    }

    console.log('Sending message:', JSON.stringify(message))

    const userMessage: ChatMessage = {
      uid: Date.now().toString(),
      vizier_session: {
        agent_id: agentId,
        channel: 'vizier-webui',
        topic: topicId,
      },
      content: {
        Request: {
          timestamp: new Date().toISOString(),
          user: username,
          content: { chat: input.trim() },
        },
      },
    }

    setMessages(prev => [...prev, userMessage])
    setInput('')
    if (inputRef.current) {
      inputRef.current.style.height = 'auto'
    }
    console.log('Calling sendJsonMessage with:', message)
    console.log('WebSocket readyState:', readyState)
    sendJsonMessage(message)
    console.log('sendJsonMessage called, readyState now:', readyState)
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
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {topicDetail ? topicDetail.title : topicId}
            </ReactMarkdown>

          </h3>
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
        {messages.length === 0 && inlineEvents.length === 0 ? (
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
              let stats: VizierResponseStats | undefined

              if (isUserMessage && msg.content.Request) {
                const request = msg.content.Request as any
                if (request.content?.chat) {
                  content = request.content.chat
                }
                senderName = request.user || 'You'
              } else if (!isUserMessage && msg.content.Response) {
                const response = msg.content.Response as any
                if (response?.content?.message?.content) {
                  content = response.content.message.content
                }
                senderName = agentDetail?.name || 'Agent'
                stats = response?.content?.message?.stats as VizierResponseStats | undefined
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
                  <div style={{
                    padding: '12px 16px',
                    background: isUserMessage ? 'var(--surface)' : 'transparent',
                    borderRadius: '8px',
                    borderLeft: isUserMessage ? 'none' : '3px solid var(--accent-primary)',
                    boxShadow: isUserMessage ? 'var(--shadow-sm)' : 'none',
                  }}>
                    <div className="flex items-start justify-between">
                      <div className='prose'>
                        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>{content}</ReactMarkdown>
                      </div>
                      <button
                        onClick={() => {
                          navigator.clipboard.writeText(content)
                          addToast('success', 'Copied!', 'Message copied to clipboard')
                        }}
                        className='sticky border flex items-center justify-center mt-1!'
                        style={{
                          color: 'var(--text-tertiary)',
                        }}
                        title="Copy to clipboard"
                      >
                        <FiCopy size={14} />
                      </button>
                    </div>

                    {!isUserMessage && stats && (
                      <div
                        title={`Input: ${stats.total_input_tokens} | Output: ${stats.total_output_tokens} | Cached: ${stats.total_cached_input_tokens} `}
                        style={{
                          marginTop: '8px',
                          padding: '4px 8px',
                          background: 'var(--surface)',
                          borderRadius: '4px',
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: '8px',
                          fontSize: '11px',
                          color: 'var(--text-tertiary)',
                        }}
                      >
                        <span>{stats.total_tokens} tokens</span>
                        <span style={{ opacity: 0.5 }}>·</span>
                        <span>in: {stats.total_input_tokens}</span>
                        <span style={{ opacity: 0.5 }}>·</span>
                        <span>out: {stats.total_output_tokens}</span>
                        <span style={{ opacity: 0.5 }}>·</span>
                        <span>{Math.round(stats.duration.secs * 1000 + stats.duration.nanos / 1000000)}ms</span>
                      </div>
                    )}
                  </div>
                </div>
              )
            })}

            {/* Thinking indicator with inline events */}
            {inlineEvents.length > 0 && (
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
                  flexDirection: 'column',
                  gap: '8px',
                  color: 'var(--text-secondary)',
                  background: 'var(--surface)',
                }}>
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px',
                    color: 'var(--text-tertiary)',
                  }}>
                    thinking
                    <div className="thinking-dots">
                      <span>.</span>
                      <span>.</span>
                      <span>.</span>
                    </div>
                  </div>
                  {inlineEvents.map((evt) => (
                    <div key={evt.id} style={{
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: '8px',
                      fontSize: '14px',
                    }}>
                      {evt.type === 'tool_choice' && evt.content && (
                        <div className="prose">
                          <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                            {evt.content}
                          </ReactMarkdown>
                        </div>
                      )}
                      {evt.type === 'thinking' && evt.content && (
                        <div className="prose">
                          <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                            {evt.content.split('\n').map(line => `> ${line} `).join('\n')}
                          </ReactMarkdown>
                        </div>
                      )}
                    </div>
                  ))}
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
        <style>{textareaStyle}</style>
        <form onSubmit={handleSendMessage} style={{
          display: 'flex',
          gap: '12px',
          maxWidth: '900px',
          margin: '0 auto',
        }}>
          <textarea
            className="chat-textarea"
            ref={inputRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value)
              e.target.style.height = 'auto'
              e.target.style.height = Math.min(e.target.scrollHeight, window.innerHeight * 0.5) + 'px'
            }}
            onKeyDown={handleKeyDown}
            placeholder={readyState === ReadyState.OPEN ? "Type your message..." : "Connecting..."}
            disabled={readyState !== ReadyState.OPEN}
            rows={1}
            style={{
              flex: 1,
              resize: 'none',
              minHeight: '44px',
              maxHeight: '50vh',
              overflowY: 'auto',
            }}
          />
          <button
            type="submit"
            className="btn btn-primary"
            disabled={!input.trim() || readyState !== ReadyState.OPEN}
            style={{ width: '44px', height: '44px', padding: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}
          >
            <FaPaperPlane />
          </button>
        </form>
      </div >
    </>
  )
}
