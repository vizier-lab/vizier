import { useEffect, useState, useRef, useCallback } from 'react'
import type { FormEvent } from 'react'
import { useParams, useNavigate } from 'react-router'
import {
  getTopicHistory,
  listTopics,
  deleteTopic,
  getAgentDetail,
  listAgents,
} from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import type {
  Agent,
  ChatMessage,
  Topic,
  VizierAttachment,
  WebSocketMessage,
  WebSocketResponse,
  VizierResponseStats,
} from '../interfaces/types'
import { getCurrentUsername } from '../utils/auth'
import { Skeleton, SkeletonMessage } from '../components/Skeleton'
import {
  FaPaperPlane,
  FaXmark,
  FaChevronDown,
  FaTrash,
  FaCloudArrowUp,
} from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import { useConnectionStore } from '../hooks/connectionStore'
import { MessageItem } from '../components/MessageItem'
import { ThinkingIndicator } from '../components/ThinkingIndicator'
import MarkdownEditor from '../components/MarkdownEditor'
import { useMeasure } from '@uidotdev/usehooks'

interface InlineEvent {
  id: string
  type: 'start' | 'tool_choice' | 'thinking'
  content?: string
  timestamp: number
}

const formatToolChoice = (
  name: string,
  args: Record<string, unknown>,
  agentNames: Record<string, string>
): string => {
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
  const [clearKey, setClearKey] = useState(0)
  const [loading, setLoading] = useState(true)
  const [inlineEvents, setInlineEvents] = useState<InlineEvent[]>([])
  const [agentDetail, setAgentDetail] = useState<Agent | null>(null)
  const [attachments, setAttachments] = useState<VizierAttachment[]>([])
  const [showSessionDropdown, setShowSessionDropdown] = useState(false)
  const [sessionList, setSessionList] = useState<Topic[]>([])
  const [showNewSessionInput, setShowNewSessionInput] = useState(false)
  const [newSessionId, setNewSessionId] = useState('')
  const [isDragOver, setIsDragOver] = useState(false)
  const [sendPulse, setSendPulse] = useState(false)
  const [imagePreviews, setImagePreviews] = useState<Record<string, string>>(
    {}
  )
  const prevInputRef = useRef('')
  const currentInputRef = useRef('')
  const dragCounterRef = useRef(0)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const thinkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  )
  const sessionSelectorRef = useRef<HTMLDivElement>(null)
  const { addToast } = useToastStore()
  const {
    connected,
    lastMessage,
    // messageCount,
    sendMessage,
    // clearLastMessage,
  } = useConnectionStore()

  const [pageRef, { width: pageWidth }] = useMeasure()
  const [inputRef, { height: inputHeight }] = useMeasure()

  const resolvedTopicId = topicId ?? 'DEFAULT'

  // Pulse send button when input transitions from empty to non-empty
  useEffect(() => {
    if (input.trim() && !prevInputRef.current.trim()) {
      setSendPulse(true)
      const timer = setTimeout(() => setSendPulse(false), 300)
      return () => clearTimeout(timer)
    }
    prevInputRef.current = input
  }, [input])

  // Cleanup image preview object URLs on unmount
  useEffect(() => {
    return () => {
      Object.values(imagePreviews).forEach((url) =>
        URL.revokeObjectURL(url)
      )
    }
  }, [])

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
    listTopics(agentId).then((topic) => {
      const detail = topic.data.find(
        (item: any) => item.topic_id === resolvedTopicId
      )
      setTopicDetail(detail || null)
    })
  }, [agentId, resolvedTopicId])

  // Clear inline events and attachments when topic changes
  useEffect(() => {
    setInlineEvents([])
    setAttachments([])
    setImagePreviews((prev) => {
      Object.values(prev).forEach((url) => URL.revokeObjectURL(url))
      return {}
    })
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

    getAgentDetail(agentId).then((data) => {
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
    listAgents().then((res) => {
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
    setInlineEvents((prev) => [
      ...prev,
      {
        id:
          Date.now().toString() +
          Math.random().toString(36).substr(2, 9),
        type,
        content,
        timestamp: Date.now(),
      },
    ])
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
        setInlineEvents([
          {
            id: Date.now().toString(),
            type: 'start',
            timestamp: Date.now(),
          },
        ])
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
        const toolContent = formatToolChoice(
          content.tool_choice.name,
          content.tool_choice.args,
          agentNames
        )
        addInlineEvent('tool_choice', toolContent)
        return
      }

      if ('message' in content) {
        clearInlineEvents()
        setMessages((prev) => {
          if (
            prev.some(
              (m) => m.content.Response?.timestamp === timestamp
            )
          ) {
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
      if (
        sessionSelectorRef.current &&
        !sessionSelectorRef.current.contains(e.target as Node)
      ) {
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

  const handleSelectSession = useCallback(
    (sessionId: string) => {
      setShowSessionDropdown(false)
      navigate(`/${agentId}/chat/${sessionId}`)
    },
    [agentId, navigate]
  )

  const handleNewSession = useCallback(() => {
    setShowSessionDropdown(false)
    setShowNewSessionInput(true)
    setNewSessionId('')
  }, [])

  const handleCreateSession = useCallback(() => {
    if (!agentId) return
    const slug = autoCorrectSlugStrict(newSessionId)
    if (!slug) return
    const exists = sessionList.some((s) => s.topic_id === slug)
    if (exists) {
      addToast(
        'error',
        'Session already exists',
        `"${slug}" already exists.`
      )
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

  const handleDeleteSession = useCallback(
    async (e: React.MouseEvent, sessionId: string) => {
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
        addToast(
          'error',
          'Failed to delete session',
          err?.response?.data?.message || err?.message
        )
      }
    },
    [agentId, resolvedTopicId, navigate, addToast, loadSessionList]
  )

  const handleSendMessage = useCallback(
    async (e: FormEvent) => {
      e.preventDefault()
      const currentInput = currentInputRef.current
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
            attachments:
              attachments.length > 0 ? attachments : undefined,
          },
        },
      }

      setMessages((prev) => [...prev, userMessage])
      setInput('')
      currentInputRef.current = ''
      setClearKey((k) => k + 1)
      setAttachments([])
      setImagePreviews((prev) => {
        Object.values(prev).forEach((url) => URL.revokeObjectURL(url))
        return {}
      })
      sendMessage(message)
    },
    [agentId, resolvedTopicId, connected, sendMessage, attachments]
  )

  const handleEditorChange = useCallback((value: string) => {
    setInput(value)
    currentInputRef.current = value
  }, [])

  const handleAttachClick = useCallback(
    () => fileInputRef.current?.click(),
    []
  )

  const handleCopyMessage = useCallback(
    (content: string) => {
      navigator.clipboard.writeText(content)
      addToast('success', 'Copied!', 'Message copied to clipboard')
    },
    [addToast]
  )

  const fileToBase64 = useCallback((file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => {
        const result = reader.result as string
        const base64 = result.split(',')[1] || ''
        resolve(base64)
      }
      reader.onerror = reject
      reader.readAsDataURL(file)
    })
  }, [])

  const processFiles = useCallback(
    async (files: File[]) => {
      const imageFiles = files.filter((f) => f.type.startsWith('image/'))
      const newPreviews: Record<string, string> = {}
      for (const file of imageFiles) {
        newPreviews[file.name] = URL.createObjectURL(file)
      }
      setImagePreviews((prev) => ({ ...prev, ...newPreviews }))

      for (const file of files) {
        try {
          const base64 = await fileToBase64(file)
          const newAttachment: VizierAttachment = {
            filename: file.name,
            content: { base64 },
          }
          setAttachments((prev) => [...prev, newAttachment])
        } catch (err: any) {
          console.error('File read failed:', err)
          addToast(
            'error',
            'File read failed',
            err?.message || 'Failed to read file'
          )
        }
      }
    },
    [addToast, fileToBase64]
  )

  const handleFileSelect = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files
      if (!files) return
      await processFiles(Array.from(files))
      if (fileInputRef.current) {
        fileInputRef.current.value = ''
      }
    },
    [processFiles]
  )

  const handleRemoveAttachment = useCallback((index: number) => {
    setAttachments((prev) => {
      const removed = prev[index]
      if (removed) {
        setImagePreviews((p) => {
          const copy = { ...p }
          delete copy[removed.filename]
          return copy
        })
      }
      return prev.filter((_, i) => i !== index)
    })
  }, [])

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragCounterRef.current++
    if (e.dataTransfer.types.includes('Files')) {
      setIsDragOver(true)
    }
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragCounterRef.current--
    if (dragCounterRef.current === 0) {
      setIsDragOver(false)
    }
  }, [])

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      dragCounterRef.current = 0
      setIsDragOver(false)
      const files = Array.from(e.dataTransfer.files).filter((f) => {
        const ext = f.name.toLowerCase()
        return (
          f.type.startsWith('image/') ||
          ext.endsWith('.pdf') ||
          ext.endsWith('.doc') ||
          ext.endsWith('.docx') ||
          ext.endsWith('.txt')
        )
      })
      if (files.length > 0) {
        processFiles(files)
      }
    },
    [processFiles]
  )

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      const items = Array.from(e.clipboardData.items)
      const imageItems = items.filter((item) =>
        item.type.startsWith('image/')
      )
      if (imageItems.length === 0) return

      e.preventDefault()
      const files = imageItems
        .map((item) => item.getAsFile())
        .filter((f): f is File => f !== null)
      if (files.length > 0) {
        processFiles(files)
        addToast(
          'info',
          'Image pasted',
          'Image uploaded from clipboard'
        )
      }
    },
    [processFiles, addToast]
  )

  if (loading) {
    return (
      <>
        <div className="main-header">
          <Skeleton variant="text" width={200} height={24} />
        </div>
        <div
          className="main-body"
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '2rem',
          }}
        >
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
        <div
          className="session-selector-wrapper"
          ref={sessionSelectorRef}
        >
          <div
            className="session-selector"
            onClick={handleToggleSessionDropdown}
          >
            <span className="session-selector-title">
              {topicDetail ? topicDetail.title : resolvedTopicId}
            </span>
            <FaChevronDown
              size={14}
              className={`session-selector-chevron ${showSessionDropdown ? 'open' : ''}`}
            />
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
                <div className="session-dropdown-empty">
                  No sessions
                </div>
              ) : (
                sessionList.map((session) => (
                  <div
                    key={session.topic_id}
                    className={`session-dropdown-item ${resolvedTopicId === session.topic_id ? 'active' : ''}`}
                    onClick={() =>
                      handleSelectSession(
                        session.topic_id
                      )
                    }
                  >
                    <div className="session-dropdown-item-info">
                      <span className="session-dropdown-item-id">
                        {session.topic_id}
                      </span>
                      {session.title &&
                        session.title !==
                        session.topic_id && (
                          <span className="session-dropdown-item-title">
                            {session.title}
                          </span>
                        )}
                    </div>
                    <button
                      className="session-dropdown-delete"
                      onClick={(e) =>
                        handleDeleteSession(
                          e,
                          session.topic_id
                        )
                      }
                      title="Delete session"
                    >
                      <FaTrash size={14} />
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
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '1.5rem',
              }}
            >
              <h2 style={{ margin: 0 }}>New Session</h2>
              <button
                className="btn btn-ghost"
                onClick={handleCancelNewSession}
                style={{ padding: '8px' }}
              >
                ✕
              </button>
            </div>

            <div className="input-group">
              <label htmlFor="new-session-id">Topic ID</label>
              <input
                id="new-session-id"
                type="text"
                value={newSessionId}
                onChange={(e) =>
                  setNewSessionId(
                    autoCorrectSlug(e.target.value)
                  )
                }
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleCreateSession()
                  if (e.key === 'Escape')
                    handleCancelNewSession()
                }}
                placeholder="my-session-name"
                autoFocus
              />
              {newSessionId && (
                <div
                  style={{
                    fontSize: '12px',
                    color: 'var(--text-tertiary)',
                    marginTop: '4px',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  →{' '}
                  {autoCorrectSlugStrict(newSessionId) ||
                    '...'}
                </div>
              )}
            </div>

            <div
              style={{
                display: 'flex',
                gap: '8px',
                marginTop: '1.5rem',
              }}
            >
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
        className="h-full overflow-y-scroll no-scrollbar w-full main-body"
        ref={pageRef}
      >
        <div
          className="no-scrollbar "
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '1.5rem',
          }}
        >
          {messages.length === 0 && inlineEvents.length === 0 ? (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                height: '100%',
                color: 'var(--text-tertiary)',
                flexDirection: 'column',
                gap: '1rem',
              }}
            >
              <div style={{ fontSize: '48px', opacity: 0.5 }}>
                💬
              </div>
              <p>No messages yet. Start the conversation!</p>
            </div>
          ) : (
            <>
              {messages.map((msg) => {
                const isUserMessage =
                  msg.content.Request !== undefined
                let content: string | undefined
                let senderName: string = 'Unknown'
                let stats: VizierResponseStats | undefined
                let msgAttachments:
                  | VizierAttachment[]
                  | undefined

                if (isUserMessage && msg.content.Request) {
                  const request = msg.content.Request as any
                  if (request.content?.chat) {
                    content = request.content.chat
                  }
                  senderName = request.user || 'You'
                  msgAttachments = request.attachments
                } else if (
                  !isUserMessage &&
                  msg.content.Response
                ) {
                  const response = msg.content.Response as any
                  if (response?.content?.message?.content) {
                    content =
                      response.content.message.content
                  }
                  senderName = agentDetail?.name || 'Agent'
                  stats = response?.content?.message
                    ?.stats as
                    | VizierResponseStats
                    | undefined
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
          <div style={{ height: `${inputHeight}px` }}></div>
        </div>
      </div>

      {/* Input */}
      <div className="no-scrollbar">
        <div
          ref={inputRef}
          className="absolute bottom-0 shadow-2xs bg-linear-to-t from-background from-30% to-transparent"
          style={{
            width: `${pageWidth}px`,
            padding: '1rem 1.5rem 1rem',
          }}
        >
          <div
            style={{
              minWidth: '90%',
              margin: '0 auto',
              display: 'flex',
              flexDirection: 'column',
              gap: '0.5rem',
              width: '100%',
            }}
          >
            {/* Input container */}
            <div
              className={`chat-input-container${isDragOver ? ' drag-over' : ''}`}
              onDragEnter={handleDragEnter}
              onDragLeave={handleDragLeave}
              onDragOver={handleDragOver}
              onDrop={handleDrop}
              onPaste={handlePaste}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                  e.preventDefault()
                  handleSendMessage(e as any)
                }
              }}
              style={{
                backdropFilter: 'blur(5px)',
                position: 'relative',
              }}
            >
              {isDragOver && (
                <div className="chat-drop-overlay">
                  <FaCloudArrowUp size={20} />
                  Drop files here
                </div>
              )}
              <form
                onSubmit={handleSendMessage}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                }}
              >
                <input
                  type="file"
                  ref={fileInputRef}
                  onChange={handleFileSelect}
                  multiple
                  accept="image/*,.pdf,.doc,.docx,.txt"
                  style={{ display: 'none' }}
                />
                <MarkdownEditor
                  key={clearKey}
                  value={input}
                  onChange={handleEditorChange}
                  onAttach={handleAttachClick}
                  className="chat-mdx-editor"
                  placeholder={connected ? 'Type a message...' : 'Connecting...'}
                  disabled={!connected}
                />
                {/* Bottom bar: chips + hint + send */}
                <div className="chat-input-bottom-bar">
                  {/* Attachment chips */}
                  {attachments.length > 0 && (
                    <div className="chat-input-chips">
                      {attachments.map((att, idx) => {
                        const isImage =
                          /\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i.test(
                            att.filename
                          )
                        const preview = imagePreviews[att.filename]
                        const base64 =
                          'base64' in att.content
                            ? att.content.base64
                            : undefined
                        const ext =
                          att.filename
                            .split('.')
                            .pop()
                            ?.toLowerCase() || 'png'
                        const mimeMap: Record<string, string> = {
                          jpg: 'image/jpeg',
                          jpeg: 'image/jpeg',
                          png: 'image/png',
                          gif: 'image/gif',
                          webp: 'image/webp',
                          svg: 'image/svg+xml',
                          bmp: 'image/bmp',
                        }
                        const base64Src = base64
                          ? `data:${mimeMap[ext] || 'image/png'};base64,${base64}`
                          : undefined
                        return (
                          <div
                            key={idx}
                            className="chat-attachment-chip"
                          >
                            {isImage && preview ? (
                              <img
                                src={preview}
                                alt={att.filename}
                                className="chat-attachment-chip-thumbnail"
                              />
                            ) : isImage && base64Src ? (
                              <img
                                src={base64Src}
                                alt={att.filename}
                                className="chat-attachment-chip-thumbnail"
                              />
                            ) : null}
                            <span>{att.filename}</span>
                            <button
                              onClick={() =>
                                handleRemoveAttachment(idx)
                              }
                              className="chat-attachment-chip-remove"
                            >
                              <FaXmark size={10} />
                            </button>
                          </div>
                        )
                      })}
                      <button
                        onClick={() => {
                          setAttachments([])
                          setImagePreviews({})
                        }}
                        className="chat-clear-all-btn"
                      >
                        Clear all
                      </button>
                    </div>
                  )}
                  <div className="chat-input-bottom-row">
                    {/* Keyboard hint */}
                    <div
                      className={`chat-keyboard-hint${input.trim() ? ' visible' : ''}`}
                    >
                      <strong>Ctrl+Enter</strong> to send
                    </div>
                    <button
                      type="submit"
                      className={`chat-send-btn chat-send-btn-inline${input.trim() ? ' has-content' : ''}${sendPulse ? ' pulse' : ''}`}
                      disabled={!input.trim() || !connected}
                    >
                      <FaPaperPlane size={14} />
                    </button>
                  </div>
                </div>
              </form>
            </div>
          </div>
        </div>
      </div>
    </>
  )
}
