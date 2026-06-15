import { useEffect, useState, useRef, useCallback, useMemo } from 'react'
import type { FormEvent } from 'react'
import { useParams, useNavigate } from 'react-router'
import {
  getTopicHistory,
  listTopics,
  deleteTopic,
  getAgentDetail,
  listAgents,
  uploadFile,
  getTopicDetail,
  api_protocol,
  base_url,
} from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { blobToWavFile } from '../utils/audio'
import type {
  Agent,
  ChatMessage,
  Topic,
  VizierAttachment,
  WebSocketMessage,
  WebSocketResponse,
  VizierResponseStats,
  ReactionEntry,
  ReactionAction,
} from '../interfaces/types'
import { getCurrentUsername } from '../utils/auth'
import { Skeleton, SkeletonMessage } from '../components/Skeleton'
import {
  FaPaperPlane,
  FaXmark,
  FaChevronDown,
  FaTrash,
  FaCloudArrowUp,
  FaPlus,
  FaMicrophone,
  FaStop,
  FaVolumeHigh,
} from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import { useConnectionStore } from '../hooks/connectionStore'
import { useUserStore } from '../hooks/userStore'
import { useQuickChatStore } from '../hooks/quickChatStore'
import { MessageItem } from '../components/MessageItem'
import { ThinkingIndicator } from '../components/ThinkingIndicator'
import { CheckpointDivider } from '../components/CheckpointDivider'
import MarkdownEditor from '../components/MarkdownEditor'
import AttachmentPreviewModal from '../components/AttachmentPreviewModal'
import { useMeasure } from '@uidotdev/usehooks'

interface InlineEvent {
  id: string
  type: 'tool_choice' | 'thinking'
  content?: string
  timestamp: number
}

const PLACEHOLDERS = [
  'What counsel do you seek?',
  'How may I advise?',
  'What wisdom do you need?',
  'Speak, and I shall advise...',
  'What troubles your mind?',
  'Seeking counsel?',
  'What shall we deliberate?',
  'What knowledge do you seek?',
  'Present your inquiry...',
  'The court is yours...',
  'What matter requires attention?',
  'How may I serve?',
  'What strategy shall we devise?',
  'What decree shall I draft?',
  'The sage awaits...',
  'Ask me anything...',
  "What's on your mind?",
  'How can I help?',
  'Ask away...',
  "Let's chat...",
  "I'm all ears...",
  'Got a question?',
  'Talk to me...',
  "What's the question?",
  'Anything on your mind?',
  'Lay it on me...',
  'Surprise me...',
  'Pick my brain...',
  'Fire away...',
  "What's the plan?",
  'Ready when you are...',
]

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
    case 'memory_follow':
      return `🔗 Following links from '${args.slug as string}' (depth: ${args.depth || 1})`
    case 'memory_graph':
      return `📊 Loading knowledge graph`
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
    case 'WRITE_SOUL':
      return `📝 Updating agent notes`
    case 'READ_SOUL':
      return `📖 Reading agent notes`
    case 'WRITE_IDENTITY':
      return `🪪 Updating identity notes`
    case 'READ_IDENTITY':
      return `🪪 Reading identity notes`
    case 'WRITE_HEARTBEAT':
      return `💗 Updating heartbeat`
    case 'READ_HEARTBEAT':
      return `💗 Reading heartbeat`
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
  const consumePendingMessage = useQuickChatStore((s) => s.consumePendingMessage)

  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [clearKey, setClearKey] = useState(0)
  const [loading, setLoading] = useState(true)
  const [historyVersion] = useState(0)
  const [inlineEvents, setInlineEvents] = useState<InlineEvent[]>([])
  const [agentDetail, setAgentDetail] = useState<Agent | null>(null)
  const [attachments, setAttachments] = useState<{ file: File; previewUrl: string | null }[]>([])
  const [previewAttachment, setPreviewAttachment] = useState<VizierAttachment | null>(null)
  const [showSessionDropdown, setShowSessionDropdown] = useState(false)
  const [sessionList, setSessionList] = useState<Topic[]>([])
  const [showNewSessionInput, setShowNewSessionInput] = useState(false)
  const [newSessionId, setNewSessionId] = useState('')
  const [isDragOver, setIsDragOver] = useState(false)
  const [sendPulse, setSendPulse] = useState(false)
  const [placeholderSeed, setPlaceholderSeed] = useState(() => Math.random())
  const [imagePreviews, setImagePreviews] = useState<Record<string, string>>(
    {}
  )
  const [queuedMessages, setQueuedMessages] = useState<ChatMessage[]>([])
  const [showScrollButton, setShowScrollButton] = useState(false)
  const [reactions, setReactions] = useState<Record<string, ReactionEntry[]>>({})
  const [isThinking, setIsThinking] = useState(false)
  const [recordingState, setRecordingState] = useState<'idle' | 'recording' | 'recorded'>('idle')
  const [recordedBlob, setRecordedBlob] = useState<Blob | null>(null)
  const [recordedUrl, setRecordedUrl] = useState<string | null>(null)
  const [recordingTime, setRecordingTime] = useState(0)
  const [expectAudioReply, setExpectAudioReply] = useState(false)
  const prevInputRef = useRef('')
  const currentInputRef = useRef('')
  const dragCounterRef = useRef(0)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const scrollContainerRef = useRef<HTMLDivElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const thinkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  )
  const sessionSelectorRef = useRef<HTMLDivElement>(null)
  const currentTopicRef = useRef<string | null>(null)
  const mediaRecorderRef = useRef<MediaRecorder | null>(null)
  const audioChunksRef = useRef<Blob[]>([])
  const recordingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const { user } = useUserStore()

  // Context window info from the last response's stats
  const lastResponseMsg = [...messages].reverse().find(m => m.content.Response)
  const lastResponse = lastResponseMsg?.content?.Response as any
  const lastMsgContent = lastResponse?.content?.message
  const lastMsgStats = lastMsgContent?.stats as VizierResponseStats | undefined
  const contextSize = lastMsgStats?.current_context_size
  const contextWindow = lastMsgStats?.context_window

  // Format token count for display (e.g., 14000 -> "14K", 1500000 -> "1.5M")
  const formatTokenCount = (tokens: number): string => {
    if (tokens >= 1_000_000) {
      return `${(tokens / 1_000_000).toFixed(1)}M`
    }
    if (tokens >= 1_000) {
      return `${Math.round(tokens / 1_000)}K`
    }
    return tokens.toString()
  }

  const placeholder = useMemo(
    () => PLACEHOLDERS[Math.floor(placeholderSeed * PLACEHOLDERS.length)],
    [placeholderSeed]
  )
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

  const resolvedTopicId = topicId ?? 'General'

  // Initialize currentTopicRef
  useEffect(() => {
    currentTopicRef.current = resolvedTopicId
  }, [])

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

  // Auto-send message from quick chat (via store)
  const pendingMsgRef = useRef<string | null>(null)
  useEffect(() => {
    const msg = consumePendingMessage()
    if (msg) pendingMsgRef.current = msg
  }, [])

  useEffect(() => {
    if (!connected || !agentId || !pendingMsgRef.current) return
    const msg = pendingMsgRef.current
    pendingMsgRef.current = null

    const timer = setTimeout(() => {
      const username = getCurrentUsername()
      const message: WebSocketMessage = {
        timestamp: new Date().toISOString(),
        user: username,
        content: { chat: msg },
        metadata: null as any,
      }

      const userMessage: ChatMessage = {
        uid: Date.now().toString(),
        vizier_session: {
          agent_id: agentId!,
          channel: 'vizier-webui',
          topic: resolvedTopicId,
        },
        content: {
          Request: {
            timestamp: new Date().toISOString(),
            user: username,
            content: { chat: msg },
          },
        },
      }
      setMessages((prev) => [...prev, userMessage])

      sendMessage(message)
    }, 50)
    return () => clearTimeout(timer)
  }, [connected, agentId])

  const [topicDetail, setTopicDetail] = useState<any | null>(null)
  const [agentNames, setAgentNames] = useState<Record<string, string>>({})

  // Redirect to last topic (or General) if no topicId
  useEffect(() => {
    if (agentId && !topicId) {
      const lastTopic = user?.user_id
        ? localStorage.getItem(`vizier_last_topic_${user.user_id}_${agentId}`)
        : null
      navigate(`/${agentId}/chat/${lastTopic || 'General'}`, { replace: true })
    }
  }, [agentId, topicId, navigate, user?.user_id])

  // Persist current topic per agent
  useEffect(() => {
    if (agentId && topicId && user?.user_id) {
      localStorage.setItem(`vizier_last_topic_${user.user_id}_${agentId}`, topicId)
    }
  }, [agentId, topicId, user?.user_id])

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

  // Poll for thinking state
  useEffect(() => {
    if (!agentId || !resolvedTopicId) return

    const pollThinkingState = async () => {
      try {
        const res = await getTopicDetail(agentId, resolvedTopicId)
        setIsThinking(res.data?.is_thinking ?? false)
      } catch {
        setIsThinking(false)
      }
    }

    // Initial fetch
    pollThinkingState()

    // Poll every 1000ms
    const interval = setInterval(pollThinkingState, 1000)

    return () => clearInterval(interval)
  }, [agentId, resolvedTopicId])

  // Clear inline events, attachments, and queued messages when topic changes
  useEffect(() => {
    currentTopicRef.current = resolvedTopicId
    setInlineEvents([])
    setAttachments([])
    setQueuedMessages([])
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
  }, [topicId, resolvedTopicId])

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

        const reactionsMap: Record<string, ReactionEntry[]> = {}
        for (const msg of historyMessages) {
          const reactions = msg.reactions
          if (reactions && reactions.length > 0) {
            reactionsMap[msg.uid] = reactions
          }
        }
        setReactions(reactionsMap)
      } catch (error) {
        console.error('Failed to load chat history:', error)
        setMessages([])
      } finally {
        setLoading(false)
      }
    }

    loadHistory()
  }, [agentId, resolvedTopicId, historyVersion])

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

  const handleReact = useCallback(
    (messageUid: string, emoji: string) => {
      if (!connected) return

      const currentUserId = getCurrentUsername()

      const reactionMessage = {
        reaction: {
          message_uid: messageUid,
          emoji,
          action: 'added' as const,
        },
      }

      sendMessage(reactionMessage as any)

      setReactions((prev) => {
        const existing = prev[messageUid] || []
        const pairExists = existing.some(
          (r) => r.user_id === currentUserId && r.emoji === emoji
        )

        if (pairExists) {
          return {
            ...prev,
            [messageUid]: existing.filter(
              (r) => !(r.user_id === currentUserId && r.emoji === emoji)
            ),
          }
        } else {
          return {
            ...prev,
            [messageUid]: [...existing, { user_id: currentUserId, emoji }],
          }
        }
      })
    },
    [connected, sendMessage]
  )

  // Handle incoming WebSocket messages
  useEffect(() => {
    if (!lastMessage) return

    const wsResponse = lastMessage as WebSocketResponse

    if (typeof wsResponse !== 'object' || wsResponse === null) {
      return
    }

    const { timestamp, content } = wsResponse

    switch (content) {
      case 'empty':
        setIsThinking(false)
        clearInlineEvents()
        return

      case 'abort':
        setIsThinking(false)
        clearInlineEvents()
        setQueuedMessages([])
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

      if ('checkpoint' in content) {
        // Add checkpoint to messages
        setMessages((prev) => {
          const newMessage: ChatMessage = {
            uid: `checkpoint-${timestamp}`,
            vizier_session: {
              agent_id: agentId!,
              channel: 'vizier-webui',
              topic: currentTopicRef.current!,
            },
            content: {
              Checkpoint: {
                handover: content.checkpoint.handover,
                timestamp,
              },
            },
          }
          return [...prev, newMessage]
        })
        return
      }

      if ('message' in content) {
        setIsThinking(false)
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
              topic: currentTopicRef.current!,
            },
            content: {
              Response: {
                timestamp,
                content,
                attachments: wsResponse.attachments,
              },
            },
          }
          return [...prev, newMessage]
        })
        // Move first queued message to normal messages
        setQueuedMessages((prev) => {
          if (prev.length === 0) return prev
          const [first, ...rest] = prev
          setMessages((msgs) => [...msgs, first])
          return rest
        })
        return
      }

      if ('error' in content) {
        setIsThinking(false)
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
              topic: currentTopicRef.current!,
            },
            content: {
              Response: {
                timestamp,
                content,
                attachments: wsResponse.attachments,
              },
            },
          }
          return [...prev, newMessage]
        })
        // Move first queued message to normal messages
        setQueuedMessages((prev) => {
          if (prev.length === 0) return prev
          const [first, ...rest] = prev
          setMessages((msgs) => [...msgs, first])
          return rest
        })
        return
      }

      if ('audio_reply' in content) {
        setIsThinking(false)
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
              topic: currentTopicRef.current!,
            },
            content: {
              Response: {
                timestamp,
                content,
                attachments: wsResponse.attachments,
              },
            },
          }
          return [...prev, newMessage]
        })
        // Move first queued message to normal messages
        setQueuedMessages((prev) => {
          if (prev.length === 0) return prev
          const [first, ...rest] = prev
          setMessages((msgs) => [...msgs, first])
          return rest
        })
        return
      }
    }
  }, [lastMessage, agentId])

  // Scroll detection
  const handleScroll = useCallback(() => {
    const el = scrollContainerRef.current
    if (!el) return
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    setShowScrollButton(distanceFromBottom > 200)
  }, [])

  const scrollToBottom = useCallback(() => {
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = scrollContainerRef.current.scrollHeight
    }
    setShowScrollButton(false)
  }, [])

  // Auto-scroll to bottom when near bottom
  useEffect(() => {
    if (!showScrollButton && scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = scrollContainerRef.current.scrollHeight
    }
  }, [messages, inlineEvents, queuedMessages, showScrollButton])

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
          navigate(`/${agentId}/chat/General`)
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

  const discardRecording = useCallback(() => {
    if (recordedUrl) {
      URL.revokeObjectURL(recordedUrl)
    }
    if (recordingTimerRef.current) {
      clearInterval(recordingTimerRef.current)
      recordingTimerRef.current = null
    }
    if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
      mediaRecorderRef.current.stop()
    }
    mediaRecorderRef.current = null
    audioChunksRef.current = []
    setRecordingState('idle')
    setRecordedBlob(null)
    setRecordedUrl(null)
    setRecordingTime(0)
  }, [recordedUrl])

  const startRecording = useCallback(async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      const recorder = new MediaRecorder(stream)
      audioChunksRef.current = []

      recorder.ondataavailable = (e) => {
        if (e.data.size > 0) {
          audioChunksRef.current.push(e.data)
        }
      }

      recorder.onstop = () => {
        stream.getTracks().forEach((t) => t.stop())
        if (audioChunksRef.current.length > 0) {
          const blob = new Blob(audioChunksRef.current, { type: recorder.mimeType })
          const url = URL.createObjectURL(blob)
          setRecordedBlob(blob)
          setRecordedUrl(url)
          setRecordingState('recorded')
        } else {
          setRecordingState('idle')
        }
      }

      mediaRecorderRef.current = recorder
      recorder.start()
      setRecordingState('recording')
      setRecordingTime(0)

      recordingTimerRef.current = setInterval(() => {
        setRecordingTime((t) => t + 1)
      }, 1000)
    } catch (err) {
      console.error('Microphone access denied:', err)
      addToast('error', 'Microphone access denied', 'Please allow microphone access to record voice messages.')
    }
  }, [addToast])

  const stopRecording = useCallback(() => {
    if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
      mediaRecorderRef.current.stop()
    }
    if (recordingTimerRef.current) {
      clearInterval(recordingTimerRef.current)
      recordingTimerRef.current = null
    }
  }, [])

  const handleMicClick = useCallback(() => {
    if (recordingState === 'idle') {
      startRecording()
    } else if (recordingState === 'recording') {
      stopRecording()
    } else if (recordingState === 'recorded') {
      discardRecording()
      startRecording()
    }
  }, [recordingState, startRecording, stopRecording, discardRecording])

  // Cleanup recording on unmount
  useEffect(() => {
    return () => {
      if (recordingTimerRef.current) clearInterval(recordingTimerRef.current)
      if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
        mediaRecorderRef.current.stop()
      }
      if (recordedUrl) URL.revokeObjectURL(recordedUrl)
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  const formatRecordingTime = useCallback((seconds: number) => {
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    return `${m}:${s.toString().padStart(2, '0')}`
  }, [])

  const handleSendMessage = useCallback(
    async (e: FormEvent) => {
      e.preventDefault()
      const currentInput = currentInputRef.current
      const hasText = !!currentInput.trim()
      const hasRecording = recordingState === 'recorded' && recordedBlob

      if (!hasText && !hasRecording) return
      if (!agentId) return

      if (!connected) {
        console.error('WebSocket not connected')
        return
      }

      const username = getCurrentUsername()

      // Upload attachments first
      let uploadedAttachments: VizierAttachment[] | undefined
      if (attachments.length > 0) {
        const results: VizierAttachment[] = []
        for (const att of attachments) {
          try {
            const res = await uploadFile(att.file)
            results.push({ filename: att.file.name, content: { local: res.url } })
          } catch (err) {
            console.error('File upload failed:', err)
            addToast('error', 'File upload failed', att.file.name)
          }
        }
        uploadedAttachments = results.length > 0 ? results : undefined
      }

      let message: WebSocketMessage
      let userMessageContent: ChatMessage['content']

      if (hasRecording && recordedBlob) {
        // Voice message: convert to WAV (universal format), then upload
        let audioFile: File
        try {
          audioFile = await blobToWavFile(recordedBlob)
        } catch (err) {
          console.error('Audio conversion failed:', err)
          addToast('error', 'Audio conversion failed', 'Could not convert voice message to WAV')
          return
        }
        try {
          const uploadRes = await uploadFile(audioFile)
          const audioAttachment: VizierAttachment = {
            filename: 'recording.wav',
            content: { local: uploadRes.url },
          }

          message = {
            timestamp: new Date().toISOString(),
            user: username,
            content: { audio_chat: [audioAttachment, null] },
            metadata: null as any,
            attachments: uploadedAttachments,
            expect_audio_reply: expectAudioReply || undefined,
          }

          userMessageContent = {
            Request: {
              timestamp: new Date().toISOString(),
              user: username,
              content: { audio_chat: [audioAttachment, null] },
              attachments: uploadedAttachments,
            },
          }
        } catch (err) {
          console.error('Audio upload failed:', err)
          addToast('error', 'Audio upload failed', 'Could not upload voice message')
          return
        }
      } else {
        // Regular text message
        message = {
          timestamp: new Date().toISOString(),
          user: username,
          content: { chat: currentInput.trim() },
          metadata: null as any,
          attachments: uploadedAttachments,
          expect_audio_reply: expectAudioReply || undefined,
        }

        userMessageContent = {
          Request: {
            timestamp: new Date().toISOString(),
            user: username,
            content: { chat: currentInput.trim() },
            attachments: uploadedAttachments,
          },
        }
      }

      const userMessage: ChatMessage = {
        uid: Date.now().toString(),
        vizier_session: {
          agent_id: agentId,
          channel: 'vizier-webui',
          topic: resolvedTopicId,
        },
        content: userMessageContent,
      }

      // If agent is thinking, queue the message; otherwise add to messages
      if (isThinking) {
        setQueuedMessages((prev) => [...prev, userMessage])
      } else {
        setMessages((prev) => [...prev, userMessage])
      }
      setInput('')
      currentInputRef.current = ''
      setClearKey((k) => k + 1)
      setAttachments([])
      setImagePreviews((prev) => {
        Object.values(prev).forEach((url) => URL.revokeObjectURL(url))
        return {}
      })
      // Clean up recording state
      if (recordedUrl) URL.revokeObjectURL(recordedUrl)
      setRecordingState('idle')
      setRecordedBlob(null)
      setRecordedUrl(null)
      setRecordingTime(0)
      audioChunksRef.current = []

      sendMessage(message)
      setPlaceholderSeed(Math.random())
    },
    [agentId, resolvedTopicId, connected, sendMessage, attachments, addToast, isThinking, recordingState, recordedBlob, recordedUrl]
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

  const handleAbort = useCallback(() => {
    if (!connected) return
    const username = getCurrentUsername()
    const message: WebSocketMessage = {
      timestamp: new Date().toISOString(),
      user: username,
      content: { command: 'abort' },
      metadata: null as any,
    }
    sendMessage(message)
    setQueuedMessages([])
  }, [connected, sendMessage])

  const processFiles = useCallback(
    (files: File[]) => {
      const newPreviews: Record<string, string> = {}
      const newAttachments: { file: File; previewUrl: string | null }[] = []

      for (const file of files) {
        const previewUrl = (file.type.startsWith('image/') || file.type.startsWith('video/')) ? URL.createObjectURL(file) : null
        if (previewUrl) newPreviews[file.name] = previewUrl
        newAttachments.push({ file, previewUrl })
      }

      setImagePreviews((prev) => ({ ...prev, ...newPreviews }))
      setAttachments((prev) => [...prev, ...newAttachments])
    },
    []
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
      if (removed?.previewUrl) {
        URL.revokeObjectURL(removed.previewUrl)
        setImagePreviews((p) => {
          const copy = { ...p }
          delete copy[removed.file.name]
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
          f.type.startsWith('video/') ||
          f.type.startsWith('audio/') ||
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
            <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0, flex: 1 }}>
              <span className="session-selector-title">
                {topicDetail?.title || resolvedTopicId}
              </span>
              {topicDetail?.title && (
                <span style={{ fontSize: '11px', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>
                  {resolvedTopicId}
                </span>
              )}
            </div>
            <FaChevronDown
              size={14}
              className={`session-selector-chevron ${showSessionDropdown ? 'open' : ''}`}
            />
          </div>

          {showSessionDropdown && (
            <div className="session-dropdown">
              <div
                className="session-dropdown-item"
                onClick={() => {
                  setShowSessionDropdown(false)
                  handleNewSession()
                }}
                style={{ borderBottom: '1px solid var(--border)', marginBottom: '4px' }}
              >
                <FaPlus size={14} style={{ color: 'var(--accent-primary)' }} />
                <div className="session-dropdown-item-info">
                  <span className="session-dropdown-item-id" style={{ color: 'var(--accent-primary)' }}>
                    Create New Topic
                  </span>
                </div>
              </div>
              {sessionList.length === 0 ? (
                <div className="session-dropdown-empty">
                  No topics yet
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
        className="no-scrollbar w-full main-body flex justify-center min-h-0"
        style={{ paddingTop: 0 }}
        ref={pageRef}
      >
        <div
          ref={scrollContainerRef}
          onScroll={handleScroll}
          className="no-scrollbar w-full! overflow-y-auto"
          style={{
            paddingTop: '24px',
            paddingLeft: '5%',
            paddingRight: '5%',
            paddingBottom: `${inputHeight}px`,
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
                width: '100%',
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
                // Handle checkpoint entries
                if (msg.content.Checkpoint) {
                  const cp = msg.content.Checkpoint
                  const handover = typeof cp === 'string' ? cp : cp.handover
                  const timestamp = typeof cp === 'string' ? (msg.timestamp || new Date().toISOString()) : cp.timestamp
                  return (
                    <CheckpointDivider
                      key={msg.uid}
                      handover={handover}
                      timestamp={timestamp}
                    />
                  )
                }

                const isUserMessage =
                  msg.content.Request !== undefined
                let content: string | undefined
                let senderName: string = 'Unknown'
                let stats: VizierResponseStats | undefined
                let msgAttachments:
                  | VizierAttachment[]
                  | undefined
                let isVoiceMessage = false
                let voiceSrc: string | undefined
                let audioReplySrc: string | undefined
                let isError = false

                if (isUserMessage && msg.content.Request) {
                  const request = msg.content.Request as any
                  if (request.content?.chat) {
                    content = request.content.chat
                  } else if (request.content?.audio_chat) {
                    // Voice message: [attachment, transcription]
                    const [att, transcription] = request.content.audio_chat
                    content = transcription || '🎤 Voice message'
                    isVoiceMessage = true
                    if ('local' in att.content) {
                      voiceSrc = (att.content.local.startsWith('http') ? '' : `${api_protocol}://${base_url}`) + att.content.local
                    } else if ('url' in att.content) {
                      voiceSrc = att.content.url
                    }
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
                    stats = response?.content?.message
                      ?.stats as
                      | VizierResponseStats
                      | undefined
                  } else if (response?.content?.error) {
                    isError = true
                    const errorContent = response.content.error
                    const kindStr = errorContent.kind === 'completion' ? 'Completion Error' :
                                    errorContent.kind === 'tool_timeout' ? 'Tool Timeout' :
                                    'Prompt Timeout'
                    content = `**${kindStr}**: ${errorContent.message}`
                  } else if (response?.content?.audio_reply) {
                    const [att, text, replyStats] = response.content.audio_reply
                    content = text || '🔊 Audio reply'
                    stats = replyStats as VizierResponseStats | undefined
                    if ('local' in att.content) {
                      audioReplySrc = (att.content.local.startsWith('http') ? '' : `${api_protocol}://${base_url}`) + att.content.local
                    } else if ('url' in att.content) {
                      audioReplySrc = att.content.url
                    } else if ('bytes' in att.content) {
                      // bytes are inline, create data URL
                      const bytes = new Uint8Array(att.content.bytes)
                      const blob = new Blob([bytes], { type: 'audio/wav' })
                      audioReplySrc = URL.createObjectURL(blob)
                    }
                  }
                  senderName = agentDetail?.name || 'Agent'
                  msgAttachments = response?.attachments
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
                    reactions={reactions[msg.uid]}
                    currentUserId={getCurrentUsername()}
                    onReact={handleReact}
                    onCopy={handleCopyMessage}
                    onPreviewAttachment={setPreviewAttachment}
                    isVoiceMessage={isVoiceMessage}
                    voiceSrc={voiceSrc}
                    audioReplySrc={audioReplySrc}
                    isError={isError}
                  />
                )
              })}

              {/* Thinking indicator with inline events */}
              <ThinkingIndicator
                isVisible={isThinking}
                inlineEvents={inlineEvents}
                agentName={agentDetail?.name || 'Agent'}
                onAbort={isThinking ? handleAbort : undefined}
              />

              {/* Queued messages */}
              {queuedMessages.map((msg) => {
                const request = msg.content.Request as any
                const content = request?.content?.chat || (request?.content?.audio_chat ? (request.content.audio_chat[1] || '🎤 Voice message') : null)
                if (!content) return null
                return (
                  <div key={msg.uid} style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div style={{ fontWeight: '600', fontSize: '14px', color: 'var(--text-primary)' }}>
                        {request.user || 'You'}
                      </div>
                      <span className="queued-badge">
                        <span className="queued-badge-icon">⏳</span>
                        Queued
                      </span>
                    </div>
                    <div style={{
                      padding: '12px 16px',
                      background: 'var(--surface)',
                      borderRadius: '8px',
                      boxShadow: 'var(--shadow-sm)',
                      opacity: 0.7,
                    }}>
                      <div className="prose">
                        {content}
                      </div>
                    </div>
                  </div>
                )
              })}
            </>
          )}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Input */}
      <div className="no-scrollbar">
        <div
          ref={inputRef}
          className="absolute bottom-0 shadow-2xs bg-linear-to-t from-background from-20% to-transparent"
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
            {showScrollButton && (
              <div className="scroll-to-bottom-row">
                <button
                  onClick={scrollToBottom}
                  className="scroll-to-bottom-btn"
                >
                  Scroll to Bottom
                </button>
              </div>
            )}
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
                  accept="image/*,.pdf,.doc,.docx,.txt,video/*,audio/*"
                  style={{ display: 'none' }}
                />
                {recordingState === 'idle' ? (
                  <MarkdownEditor
                    key={clearKey}
                    value={input}
                    onChange={handleEditorChange}
                    onAttach={handleAttachClick}
                    className="chat-mdx-editor"
                    placeholder={connected ? placeholder : 'Connecting...'}
                    disabled={!connected}
                  />
                ) : (
                  <div className="voice-panel">
                    {recordingState === 'recording' ? (
                      <>
                        <div className="voice-panel-dot" />
                        <span className="voice-panel-label">Recording</span>
                        <span className="voice-panel-time">{formatRecordingTime(recordingTime)}</span>
                      </>
                    ) : (
                      <>
                        <span className="voice-panel-label">🎤 Voice message</span>
                        <span className="voice-panel-time">{formatRecordingTime(recordingTime)}</span>
                      </>
                    )}
                  </div>
                )}
                {/* Bottom bar: chips + hint + send */}
                <div className="chat-input-bottom-bar">
                  {/* Attachment chips */}
                  {attachments.length > 0 && (
                    <div className="chat-input-chips">
                      {attachments.map((att, idx) => {
                        const isImage = att.file.type.startsWith('image/')
                        const isVideo = att.file.type.startsWith('video/')
                        return (
                          <div
                            key={idx}
                            className="chat-attachment-chip"
                          >
                            {isImage && att.previewUrl && (
                              <img
                                src={att.previewUrl}
                                alt={att.file.name}
                                className="chat-attachment-chip-thumbnail"
                              />
                            )}
                            {isVideo && att.previewUrl && (
                              <video
                                src={att.previewUrl}
                                className="chat-attachment-chip-thumbnail"
                                muted
                              />
                            )}
                            <span>{att.file.name}</span>
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
                          attachments.forEach((att) => {
                            if (att.previewUrl) URL.revokeObjectURL(att.previewUrl)
                          })
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
                    {/* Context window display */}
                    {contextSize !== undefined && (
                      <div
                        className="chat-context-hint visible"
                        title={contextWindow ? `Context: ${contextSize.toLocaleString()} / ${contextWindow.toLocaleString()} tokens` : `Context: ${contextSize.toLocaleString()} tokens`}
                      >
                        {contextWindow ? (
                          <>{formatTokenCount(contextSize)}/{formatTokenCount(contextWindow)} ({Math.round((contextSize / contextWindow) * 100)}%)</>
                        ) : (
                          <>{formatTokenCount(contextSize)} tokens</>
                        )}
                      </div>
                    )}
                    {/* Keyboard hint */}
                    <div
                      className={`chat-keyboard-hint${input.trim() || recordingState !== 'idle' ? ' visible' : ''}`}
                    >
                      {recordingState === 'idle' ? (
                        <><strong>Ctrl+Enter</strong> to send</>
                      ) : recordingState === 'recording' ? (
                        <span style={{ color: 'var(--text-tertiary)' }}>Click stop to finish recording</span>
                      ) : (
                        <span style={{ color: 'var(--text-tertiary)' }}>Voice message ready</span>
                      )}
                    </div>
                    <div className="chat-input-buttons">
                      {/* Mic button */}
                      {typeof MediaRecorder !== 'undefined' && recordingState === 'idle' && (
                        <button
                          type="button"
                          className="chat-mic-btn"
                          onClick={handleMicClick}
                          title="Record voice message"
                          disabled={!connected}
                        >
                          <FaMicrophone size={14} />
                        </button>
                      )}
                      {recordingState === 'recording' && (
                        <button
                          type="button"
                          className="chat-mic-btn chat-mic-btn-stop"
                          onClick={handleMicClick}
                          title="Stop recording"
                        >
                          <FaStop size={14} />
                        </button>
                      )}
                      {recordingState === 'recorded' && (
                        <button
                          type="button"
                          className="chat-mic-btn"
                          onClick={discardRecording}
                          title="Discard recording"
                        >
                          <FaXmark size={14} />
                        </button>
                      )}
                      {/* Audio reply toggle */}
                      <button
                        type="button"
                        className={`chat-mic-btn${expectAudioReply ? ' audio-reply-active' : ''}`}
                        onClick={() => setExpectAudioReply((prev) => !prev)}
                        title={expectAudioReply ? 'Audio reply ON' : 'Audio reply OFF'}
                      >
                        <FaVolumeHigh size={14} />
                      </button>
                      <button
                        type="submit"
                        className={`chat-send-btn chat-send-btn-inline${(input.trim() || recordingState !== 'idle') ? ' has-content' : ''}${sendPulse ? ' pulse' : ''}`}
                        disabled={(!input.trim() && recordingState === 'idle') || !connected}
                      >
                        <FaPaperPlane size={14} />
                      </button>
                    </div>
                  </div>
                </div>
              </form>
            </div>
          </div>
        </div>
      </div>

      <AttachmentPreviewModal
        attachment={previewAttachment}
        onClose={() => setPreviewAttachment(null)}
      />
    </>
  )
}
