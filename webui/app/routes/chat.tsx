import { useEffect, useState, useRef, useCallback, useMemo } from 'react'
import type { FormEvent } from 'react'
import { useParams, useNavigate } from 'react-router'
import { getTopicHistory, listTopics, deleteTopic, getAgentDetail, listAgents, uploadFile, base_url } from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import type { Agent, ChatMessage, Topic, VizierAttachment, WebSocketMessage, WebSocketResponse, VizierResponseStats } from '../interfaces/types'
import { getCurrentUsername } from '../utils/auth'
import { Skeleton, SkeletonMessage } from '../components/Skeleton'
import { FaPaperPlane, FaPaperclip } from 'react-icons/fa'
import { FiX, FiChevronDown, FiTrash2 } from 'react-icons/fi'
import { useToastStore } from '../hooks/toastStore'
import { useConnectionStore } from '../hooks/connectionStore'
import { MessageItem } from '../components/MessageItem'
import { ThinkingIndicator } from '../components/ThinkingIndicator'
import { debounce } from '../utils/debounce'

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

const formatToolChoice = (name: string, args: Record<string, unknown>, agentNames: Record<string, string>): string => {
  switch (name) {
    case 'think':
      return `💭 ${args.thought as string}`
    case 'memory_read':
      return `🔍 Searching memory for '${args.query as string}'`
    case 'memory_write':
      return `💾 Remembering: '${args.title as string}'`
    case 'memory_list':
      return `📚 Listing memories`
    case 'memory_detail':
      return `🔎 Getting memory detail for '${args.slug as string}'`
    case 'web_search':
      return `🌐 Searching the web for '${args.query as string}'`
    case 'news_search':
      return `📰 Finding news about '${args.query as string}'`
    case 'shell_exec':
      return `🖥️ Running shell command:\n\`\`\`bash\n${args.commands as string}\n\`\`\``
    case 'programmatic_sandbox':
      return `🐍 Running Python script:\n\`\`\`python\n${args.script as string}\n\`\`\``
    case 'schedule_one_time_task':
      return `⏰ Scheduling task: '${args.title as string}'`
    case 'schedule_cron_task':
      return `🔄 Setting up recurring task: '${args.title as string}'`
    case 'consult_agent': {
      const targetAgentId = args.agent_id as string
      const agentName = agentNames[targetAgentId] || targetAgentId
      return `🤝 Consulting agent ${agentName} about '${args.prompt as string}'`
    }
    case 'delegate_agent': {
      const targetAgentId = args.agent_id as string
      const agentName = agentNames[targetAgentId] || targetAgentId
      return `👤 Assigning task to ${agentName}: '${args.prompt as string}'`
    }
    case 'paralel_subtasks':
      return `⚡ Running parallel tasks`
    case 'create_skill':
      return `🎯 Creating skill: '${args.name as string}'`
    case 'WRITE_AGENT_MD_FILE':
      return `📝 Updating agent notes`
    case 'READ_AGENT_MD_FILE':
      return `📖 Reading agent notes`
    case 'WRITE_IDENTITY_MD_FILE':
      return `🪪 Updating identity notes`
    case 'READ_IDENTITY_MD_FILE':
      return `🪪 Reading identity notes`
    case 'WRITE_HEARTBEAT_MD_FILE':
      return `💗 Updating heartbeat`
    case 'READ_HEARTBEAT_MD_FILE':
      return `💗 Reading heartbeat`
    case 'shared_document_read':
      return `📄 Searching shared docs for '${args.query as string}'`
    case 'shared_document_write':
      return `📄 Writing shared doc: '${args.title as string}'`
    case 'shared_document_get':
      return `📄 Getting shared doc: '${args.slug as string}'`
    case 'shared_document_list':
      return `📁 Listing shared docs`
    case 'discord_send_message':
      return `💬 Sending Discord message`
    case 'discord_react_message':
      return `👍 Reacting on Discord`
    case 'discord_get_message_by_id':
      return `📩 Getting Discord message`
    case 'telegram_send_message':
      return `✈️ Sending Telegram message`
    case 'telegram_react_message':
      return `👍 Reacting on Telegram`
    case 'telegram_get_message_by_id':
      return `📩 Getting Telegram message`
    case 'discord_dm_primary_user':
      return `💬 DM on Discord`
    case 'telegram_dm_primary_user':
      return `✈️ DM on Telegram`
    case 'webui_notify_primary_user':
      return `🔔 WebUI notification`
    case 'notify_primary_user':
      return `🔔 Notifying user`
    default:
      if (name.startsWith('mcp_')) {
        const parts = name.replace('mcp_', '').split('__')
        const server = parts[0]
        const toolName = parts.slice(1).join('__')
        return `🔌 Using MCP tool: ${toolName} (${server})`
      }
      const formattedArgs = `\`\`\`js\n${JSON.stringify(args, null, 2)}\n\`\`\``
      return `🔧 Using ${name}\n${formattedArgs}`
  }
}

export default function Chat() {
  const { agentId, topicId } = useParams()
  const navigate = useNavigate()

  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(true)
  const [inlineEvents, setInlineEvents] = useState<InlineEvent[]>([])
  const [agentDetail, setAgentDetail] = useState<Agent | null>(null)
  const [attachments, setAttachments] = useState<VizierAttachment[]>([])
  const [uploading, setUploading] = useState(false)
  const [showSessionDropdown, setShowSessionDropdown] = useState(false)
  const [sessionList, setSessionList] = useState<Topic[]>([])
  const [showNewSessionInput, setShowNewSessionInput] = useState(false)
  const [newSessionId, setNewSessionId] = useState('')
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const thinkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const sessionSelectorRef = useRef<HTMLDivElement>(null)
  const { addToast } = useToastStore()
  const { connected, lastMessage, messageCount, sendMessage, clearLastMessage } = useConnectionStore()

  const resolvedTopicId = topicId ?? 'DEFAULT'

  // Create debounced resize handler to prevent layout thrashing on every keystroke
  const debouncedResize = useMemo(
    () => debounce((target: HTMLTextAreaElement) => {
      target.style.height = 'auto'
      target.style.height = Math.min(target.scrollHeight, window.innerHeight * 0.5) + 'px'
    }, 50),
    []
  )

  const [topicDetail, setTopicDetail] = useState<any | null>(null)
  const [agentNames, setAgentNames] = useState<Record<string, string>>({})

  // Redirect to DEFAULT if no topicId
  useEffect(() => {
    if (agentId && !topicId) {
      navigate(`/${agentId}/chat/DEFAULT`, { replace: true })
    }
  }, [agentId, topicId, navigate])

  // Load topic detail
  useEffect(() => {
    if (!agentId) return
    listTopics(agentId).then(topic => {
      const detail = topic.data.find((item: any) => item.topic_id === resolvedTopicId)
      setTopicDetail(detail || null)
    })
  }, [agentId, resolvedTopicId])

  // Clear inline events and attachments when topic changes
  useEffect(() => {
    setInlineEvents([])
    setAttachments([])
    setShowSessionDropdown(false)
    setShowNewSessionInput(false)
    setNewSessionId('')
    if (thinkingTimeoutRef.current) {
      clearTimeout(thinkingTimeoutRef.current)
      thinkingTimeoutRef.current = null
    }
  }, [topicId])

  // Load chat history
  useEffect(() => {
    if (!agentId) return

    getAgentDetail(agentId).then(data => {
      setAgentDetail(data.data)
    })

    const loadHistory = async () => {
      try {
        const response = await getTopicHistory(agentId, resolvedTopicId)
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
  }, [agentId, resolvedTopicId])

  useEffect(() => {
    listAgents().then(res => {
      const names: Record<string, string> = {}
      res.data.forEach((agent: Agent) => {
        names[agent.agent_id] = agent.name
      })
      setAgentNames(names)
    })
  }, [])

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
    if (!lastMessage) return

    const wsResponse = lastMessage as WebSocketResponse

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
        const toolContent = formatToolChoice(content.tool_choice.name, content.tool_choice.args, agentNames)
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
              topic: resolvedTopicId,
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
  }, [lastMessage, agentId, resolvedTopicId])

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Close session dropdown on outside click
  useEffect(() => {
    if (!showSessionDropdown) return
    const handleClick = (e: MouseEvent) => {
      if (sessionSelectorRef.current && !sessionSelectorRef.current.contains(e.target as Node)) {
        setShowSessionDropdown(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [showSessionDropdown])

  const loadSessionList = useCallback(async () => {
    if (!agentId) return
    try {
      const res = await listTopics(agentId)
      setSessionList(res.data || [])
    } catch (err) {
      console.error('Failed to load sessions:', err)
    }
  }, [agentId])

  const handleToggleSessionDropdown = useCallback(() => {
    const next = !showSessionDropdown
    setShowSessionDropdown(next)
    if (next) {
      loadSessionList()
    }
  }, [showSessionDropdown, loadSessionList])

  const handleSelectSession = useCallback((sessionId: string) => {
    setShowSessionDropdown(false)
    navigate(`/${agentId}/chat/${sessionId}`)
  }, [agentId, navigate])

  const handleNewSession = useCallback(() => {
    setShowSessionDropdown(false)
    setShowNewSessionInput(true)
    setNewSessionId('')
  }, [])

  const handleCreateSession = useCallback(() => {
    if (!agentId) return
    const slug = autoCorrectSlugStrict(newSessionId)
    if (!slug) return
    const exists = sessionList.some(s => s.topic_id === slug)
    if (exists) {
      addToast('error', 'Session already exists', `"${slug}" already exists.`)
      return
    }
    setShowSessionDropdown(false)
    setShowNewSessionInput(false)
    setNewSessionId('')
    navigate(`/${agentId}/chat/${slug}`)
  }, [agentId, newSessionId, sessionList, navigate, addToast])

  const handleCancelNewSession = useCallback(() => {
    setShowNewSessionInput(false)
    setNewSessionId('')
  }, [])

  const handleDeleteSession = useCallback(async (e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation()
    if (!agentId) return
    if (!confirm(`Delete session "${sessionId}"?`)) return
    try {
      await deleteTopic(agentId, sessionId)
      addToast('success', 'Session deleted')
      if (resolvedTopicId === sessionId) {
        navigate(`/${agentId}/chat/DEFAULT`)
      } else {
        loadSessionList()
      }
    } catch (err: any) {
      addToast('error', 'Failed to delete session', err?.response?.data?.message || err?.message)
    }
  }, [agentId, resolvedTopicId, navigate, addToast, loadSessionList])

  const handleSendMessage = useCallback(async (e: FormEvent) => {
    e.preventDefault()
    const currentInput = inputRef.current?.value || ''
    if (!currentInput.trim() || !agentId) return

    if (!connected) {
      console.error('WebSocket not connected')
      return
    }

    const username = getCurrentUsername()

    const message: WebSocketMessage = {
      timestamp: new Date().toISOString(),
      user: username,
      content: { chat: currentInput.trim() },
      metadata: null as any,
      attachments: attachments.length > 0 ? attachments : undefined,
    }

    const userMessage: ChatMessage = {
      uid: Date.now().toString(),
      vizier_session: {
        agent_id: agentId,
        channel: 'vizier-webui',
        topic: resolvedTopicId,
      },
      content: {
        Request: {
          timestamp: new Date().toISOString(),
          user: username,
          content: { chat: currentInput.trim() },
          attachments: attachments.length > 0 ? attachments : undefined,
        },
      },
    }

    setMessages(prev => [...prev, userMessage])
    setInput('')
    setAttachments([])
    if (inputRef.current) {
      inputRef.current.style.height = 'auto'
    }
    sendMessage(message)
  }, [agentId, resolvedTopicId, connected, sendMessage, attachments])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSendMessage(e as any)
    }
  }, [handleSendMessage])

  const handleCopyMessage = useCallback((content: string) => {
    navigator.clipboard.writeText(content)
    addToast('success', 'Copied!', 'Message copied to clipboard')
  }, [addToast])

  const handleFileSelect = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files
    if (!files) return

    setUploading(true)
    for (const file of Array.from(files)) {
      try {
        const result = await uploadFile(file)
        const newAttachment: VizierAttachment = {
          filename: result.filename,
          content: { url: `http://${base_url}${result.url}` },
        }
        setAttachments(prev => [...prev, newAttachment])
      } catch (err: any) {
        console.error('Upload failed:', err)
        addToast('error', 'Upload failed', err?.message || 'Failed to upload file')
      }
    }
    setUploading(false)
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
    }
  }, [addToast])

  const handleRemoveAttachment = useCallback((index: number) => {
    setAttachments(prev => prev.filter((_, i) => i !== index))
  }, [])

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

  return (
    <>
      {/* Header */}
      <div className="main-header">
        <div className="session-selector-wrapper" ref={sessionSelectorRef}>
          <div
            className="session-selector"
            onClick={handleToggleSessionDropdown}
          >
            <span className="session-selector-title">
              {topicDetail ? topicDetail.title : resolvedTopicId}
            </span>
            <FiChevronDown size={14} className={`session-selector-chevron ${showSessionDropdown ? 'open' : ''}`} />
          </div>
          <button
            className="session-new-btn"
            onClick={handleNewSession}
            title="New session"
          >
            +
          </button>

          {showSessionDropdown && (
            <div className="session-dropdown">
              {sessionList.length === 0 ? (
                <div className="session-dropdown-empty">No sessions</div>
              ) : (
                sessionList.map((session) => (
                  <div
                    key={session.topic_id}
                    className={`session-dropdown-item ${resolvedTopicId === session.topic_id ? 'active' : ''}`}
                    onClick={() => handleSelectSession(session.topic_id)}
                  >
                    <div className="session-dropdown-item-info">
                      <span className="session-dropdown-item-id">{session.topic_id}</span>
                      {session.title && session.title !== session.topic_id && (
                        <span className="session-dropdown-item-title">{session.title}</span>
                      )}
                    </div>
                    <button
                      className="session-dropdown-delete"
                      onClick={(e) => handleDeleteSession(e, session.topic_id)}
                      title="Delete session"
                    >
                      <FiTrash2 size={14} />
                    </button>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>

      {/* New Session Modal */}
      {showNewSessionInput && (
        <>
          <div
            style={{
              position: 'fixed',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: 'rgba(0, 0, 0, 0.5)',
              zIndex: 1000,
              backdropFilter: 'blur(4px)',
            }}
            onClick={handleCancelNewSession}
          />
          <div
            style={{
              position: 'fixed',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
              background: 'var(--background)',
              borderRadius: '12px',
              padding: '2rem',
              maxWidth: '420px',
              width: '90%',
              zIndex: 1001,
              border: '1px solid var(--border)',
              boxShadow: 'var(--shadow-xl)',
            }}
          >
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '1.5rem',
            }}>
              <h2 style={{ margin: 0 }}>New Session</h2>
              <button className="btn btn-ghost" onClick={handleCancelNewSession} style={{ padding: '8px' }}>✕</button>
            </div>

            <div className="input-group">
              <label htmlFor="new-session-id">Topic ID</label>
              <input
                id="new-session-id"
                type="text"
                value={newSessionId}
                onChange={(e) => setNewSessionId(autoCorrectSlug(e.target.value))}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleCreateSession()
                  if (e.key === 'Escape') handleCancelNewSession()
                }}
                placeholder="my-session-name"
                autoFocus
              />
              {newSessionId && (
                <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px', fontFamily: 'var(--font-mono)' }}>
                  → {autoCorrectSlugStrict(newSessionId) || '...'}
                </div>
              )}
            </div>

            <div style={{ display: 'flex', gap: '8px', marginTop: '1.5rem' }}>
              <button
                className="btn btn-primary"
                onClick={handleCreateSession}
                disabled={!autoCorrectSlugStrict(newSessionId)}
                style={{ flex: 1, justifyContent: 'center' }}
              >
                Create
              </button>
              <button
                className="btn btn-secondary"
                onClick={handleCancelNewSession}
              >
                Cancel
              </button>
            </div>
          </div>
        </>
      )}

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
              let msgAttachments: VizierAttachment[] | undefined

              if (isUserMessage && msg.content.Request) {
                const request = msg.content.Request as any
                if (request.content?.chat) {
                  content = request.content.chat
                }
                senderName = request.user || 'You'
                msgAttachments = request.attachments
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
                <MessageItem
                  key={msg.uid}
                  uid={msg.uid}
                  isUserMessage={isUserMessage}
                  senderName={senderName}
                  content={content}
                  stats={stats}
                  attachments={msgAttachments}
                  onCopy={handleCopyMessage}
                />
              )
            })}

            {/* Thinking indicator with inline events */}
            <ThinkingIndicator
              inlineEvents={inlineEvents}
              agentName={agentDetail?.name || 'Agent'}
            />
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
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '12px',
          maxWidth: '900px',
          margin: '0 auto',
        }}>
          {/* Attachment chips */}
          {attachments.length > 0 && (
            <div style={{
              display: 'flex',
              flexWrap: 'wrap',
              gap: '8px',
              alignItems: 'center',
            }}>
              <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>Attached:</span>
              {attachments.map((att, idx) => (
                <div
                  key={idx}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '4px 8px',
                    background: 'var(--surface)',
                    borderRadius: '4px',
                    fontSize: '12px',
                  }}
                >
                  <span>{att.filename}</span>
                  <button
                    onClick={() => handleRemoveAttachment(idx)}
                    style={{
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      padding: 0,
                      display: 'flex',
                      alignItems: 'center',
                      color: 'var(--text-tertiary)',
                    }}
                  >
                    <FiX size={12} />
                  </button>
                </div>
              ))}
              <button
                onClick={() => setAttachments([])}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: '11px',
                  color: 'var(--text-tertiary)',
                  textDecoration: 'underline',
                }}
              >
                Clear all
              </button>
            </div>
          )}
          {/* Input row */}
          <form onSubmit={handleSendMessage} style={{ display: 'flex', gap: '12px' }}>
            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileSelect}
              multiple
              accept="image/*,.pdf,.doc,.docx,.txt"
              style={{ display: 'none' }}
            />
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              disabled={!connected || uploading}
              style={{
                width: '44px',
                height: '44px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: '8px',
                cursor: !connected || uploading ? 'not-allowed' : 'pointer',
                color: !connected || uploading ? 'var(--text-tertiary)' : 'var(--text-primary)',
                opacity: uploading ? 0.7 : 1,
              }}
              title="Attach file"
            >
              {uploading ? (
                <span style={{ fontSize: '10px' }}>...</span>
              ) : (
                <FaPaperclip />
              )}
            </button>
            <textarea
              className="chat-textarea"
              ref={inputRef}
              value={input}
              onChange={(e) => {
                setInput(e.target.value)
                debouncedResize(e.target)
              }}
              onKeyDown={handleKeyDown}
              placeholder={connected ? "Type a message..." : "Connecting..."}
              disabled={!connected}
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
              disabled={!input.trim() || !connected}
              style={{ width: '44px', height: '44px', padding: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}
            >
              <FaPaperPlane />
            </button>
          </form>
        </div>
      </div >
    </>
  )
}
