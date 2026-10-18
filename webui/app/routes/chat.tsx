import { Fragment, useEffect, useState, useRef, useCallback, useMemo } from 'react'
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
  getPieceHistory,
  listAvailableSkills,
  api_protocol,
  base_url,
} from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { blobToWavFile } from '../utils/audio'
import type {
  Agent,
  BackgroundReport,
  ChatMessage,
  ClientCallView,
  Topic,
  TrailEvent,
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
import {
  clientEventMessage,
  subscribeClientFrames,
  subscribeReactionFrames,
  useConnectionStore,
} from '../hooks/connectionStore'
import { useUserStore } from '../hooks/userStore'
import { useQuickChatStore } from '../hooks/quickChatStore'
import { MessageItem } from '../components/MessageItem'
import { ThinkingIndicator } from '../components/ThinkingIndicator'
import ActivityTrail from '../components/ActivityTrail'
import {
  appendClientCall,
  appendLiveEvent,
  applyClientOutcome,
  applyClientState,
  groupHistory,
  outcomeDurationMs,
} from '../lib/trail'
import { findClientTool, visibleClientCalls } from '../lib/clientTools'
import type { ClientCallContext } from '../lib/clientTools'
import { ClientCallChip } from '../components/client/ClientCallChip'
import { CheckpointDivider } from '../components/CheckpointDivider'
import { BackgroundReportItem } from '../components/BackgroundReportItem'
import { BackgroundJobTray } from '../components/BackgroundJobTray'
import SlideOver from '../components/SlideOver'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { useBackgroundJobStore } from '../hooks/backgroundJobStore'
import MarkdownEditor from '../components/MarkdownEditor'
import AttachmentPreviewModal from '../components/AttachmentPreviewModal'
import { useMeasure } from '@uidotdev/usehooks'

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

// How long after the socket (or a send) changes the thinking state a poll is too stale to
// overrule it.
const THINKING_POLL_GRACE_MS = 2000

type SlashSuggestion = { name: string; description: string; skill?: boolean }

const SLASH_COMMANDS: SlashSuggestion[] = [
  { name: '/checkpoint', description: 'Save checkpoint with handover' },
  { name: '/lobotomy', description: 'Save checkpoint without handover' },
  { name: '/abort', description: 'Abort current thinking' },
  { name: '/skills', description: 'List the skills you can run' },
]

// The editor writes markdown, which escapes `_`; a command's name must reach the agent as
// typed (`/release_notes`, not `/release\_notes`).
const unescapeCommand = (text: string): string =>
  text.replace(/^\/\S*/, (head) => head.replace(/\\_/g, '_'))

// Exported so the task view can label a run's trail with the same catalogue. One home for
// the tool names, rather than a second list that drifts from this one.
export const formatToolChoice = (
  name: string,
  args: Record<string, unknown>,
  agentNames: Record<string, string>
): string => {
  switch (name) {
    case 'think':
      return `💭 ${args.thought as string}`
    case 'list_tool_functions':
      return `📖 Listing tool functions`
    case 'describe_tool_function':
      return `📖 Describing \`${args.name as string}\``
    case 'memory_read':
      return `🔍 Searching memory for '${args.query as string}'`
    case 'memory_write':
      return `💾 Remembering: '${args.title as string}'`
    case 'memory_list': {
      const limit = (args.limit as number) || 50
      const offset = (args.offset as number) || 0
      return `📚 Listing memories (showing ${limit} starting at ${offset})`
    }
    case 'memory_detail':
      return `🔎 Getting memory detail for '${args.slug as string}'`
    case 'memory_follow':
      return `🔗 Following links from '${args.slug as string}' (depth: ${args.depth || 1})`
    case 'memory_graph': {
      const tags = args.tags as string[] | undefined
      if (tags && tags.length > 0) {
        return `📊 Loading knowledge graph (tags: [${tags.join(', ')}])`
      }
      return `📊 Loading knowledge graph`
    }
    case 'memory_delete':
      return `🗑️ Deleting memory '${args.slug as string}'`
    case 'brave_search_web':
      return `🌐 Searching the web for '${args.query as string}'`
    case 'brave_search_news':
      return `📰 Finding news about '${args.query as string}'`
    case 'shell_exec':
      return `🖥️ Running shell command:\n\`\`\`bash\n${args.commands as string}\n\`\`\``
    case 'list_task': {
      const isActive = args.is_active as boolean | undefined
      if (isActive !== undefined) {
        return `📋 Listing tasks (filter: active=${isActive})`
      }
      return `📋 Listing all tasks`
    }
    case 'delete_task':
      return `🗑️ Deleting task '${args.slug as string}'`
    case 'get_task_detail':
      return `📋 Getting task detail for '${args.slug as string}'`
    case 'list_task_runs':
      return `🕘 Listing past runs of task '${args.slug as string}'`
    case 'get_task_run_detail':
      return `📄 Reading the report from the ${args.run_id as string} run of '${args.slug as string}'`
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
    case 'update_skill':
      return `✏️ Updating skill '${args.slug as string}'`
    case 'delete_skill':
      return `🗑️ Deleting skill '${args.slug as string}'`
    case 'list_skills': {
      const keyword = args.keyword as string | undefined
      if (keyword) {
        return `📚 Listing skills (keyword: '${keyword}')`
      }
      return `📚 Listing all skills`
    }
    case 'get_skill_details':
      return `🔍 Getting skill details '${args.slug as string}'`
    case 'use_skill':
      return `🧩 Using skill '${args.slug as string}'`
    case 'read_skill_resource':
      return `📖 Reading skill resource '${args.slug as string}/${args.path as string}'`
    case 'execute_skill_resource':
      return `▶️ Executing skill resource '${args.slug as string}/${args.path as string}'`
    case 'list_session_files':
      return `📂 Listing session files`
    case 'read_document_file':
      return `📄 Reading document '${args.filename as string}'`
    case 'send_attachment':
      return `📎 Sending attachment '${args.filename as string}'`
    case 'fetch':
      return `🌐 Fetching '${args.url as string}'`
    case 'http_client':
      return `🔗 HTTP ${(args.method as string) || 'GET'} '${args.url as string}'`
    case 'tts_generate': {
      const text = (args.text as string) || ''
      const truncated = text.length > 60 ? `${text.slice(0, 60)}...` : text
      return `🔊 Generating speech: '${truncated}'`
    }
    case 'stt_transcribe':
      return `🎤 Transcribing '${args.filename as string}'`
    case 'image_generate': {
      const prompt = (args.prompt as string) || ''
      const truncated = prompt.length > 60 ? `${prompt.slice(0, 60)}...` : prompt
      return `🎨 Generating image: '${truncated}'`
    }
    case 'read_image_file':
      return `🖼️ Reading image '${args.filename as string}'`
    case 'write_dream_journal':
      return `💤 Writing dream journal (${args.stage as string})`
    case 'read_dream_journal': {
      const filters: string[] = []
      if (args.stage) filters.push(`stage: ${args.stage as string}`)
      if (args.cycle_id) filters.push(`cycle: ${args.cycle_id as string}`)
      if (args.limit) filters.push(`limit: ${args.limit as number}`)
      return `💤 Reading dream journal${filters.length > 0 ? ` (${filters.join(', ')})` : ''}`
    }
    case 'write_core':
      return `🧠 Updating core`
    case 'read_core':
      return `🧠 Reading core`
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
    case 'webui_send_message':
      return `💬 Sending WebUI message to @${args.username as string}`
    case 'webui_list_topics':
      return `📋 Listing WebUI topics for @${args.username as string}`
    default:
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
  // Bumped to re-read history without the loading skeleton: when a turn is seen to end
  // without its closing frame having reached this page (see the thinking poll).
  const [historyVersion, setHistoryVersion] = useState(0)
  // What the agent is doing in the turn that is streaming right now.
  const [liveTrail, setLiveTrail] = useState<TrailEvent[]>([])
  // The trail of every turn that has already closed in this session, keyed by the uid of
  // the entry it ended at. History is not re-read when a turn closes normally, so a finished
  // turn's trail has to come from the events already collected here, not from a reload.
  const [closedTrails, setClosedTrails] = useState<Record<string, TrailEvent[]>>({})
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
  // The latest thinking state for callbacks and effects that must not wait for a render, and
  // when the socket (or this page) last changed it: see the thinking poll.
  const isThinkingRef = useRef(false)
  const thinkingChangedAtRef = useRef(0)
  const setThinking = useCallback((value: boolean) => {
    isThinkingRef.current = value
    thinkingChangedAtRef.current = Date.now()
    setIsThinking(value)
  }, [])
  const loadedTopicRef = useRef<string | null>(null)
  // The background piece whose own conversation is open in the side panel.
  const [openPiece, setOpenPiece] = useState<{ jobId: string; ordinal: number } | null>(null)
  const [pieceHistory, setPieceHistory] = useState<ChatMessage[] | null>(null)
  const lastAppliedJob = useBackgroundJobStore((s) => s.lastApplied)
  const [recordingState, setRecordingState] = useState<'idle' | 'recording' | 'recorded'>('idle')
  const [recordedBlob, setRecordedBlob] = useState<Blob | null>(null)
  const [recordedUrl, setRecordedUrl] = useState<string | null>(null)
  const [recordingTime, setRecordingTime] = useState(0)
  const [expectAudioReply, setExpectAudioReply] = useState(false)
  const [showCommandSuggestions, setShowCommandSuggestions] = useState(false)
  const [selectedCommandIndex, setSelectedCommandIndex] = useState(0)
  // Skills the agent can run with `/<name>`; offered after the built-in commands.
  const [availableSkills, setAvailableSkills] = useState<SlashSuggestion[]>([])
  // Set when a suggestion is put in the composer, so the remounted editor takes focus.
  const [focusComposer, setFocusComposer] = useState(false)
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
  const { user, loading: userLoading } = useUserStore()

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

  useEffect(() => {
    if (!agentId) return
    let cancelled = false
    listAvailableSkills(agentId)
      .then((skills) => {
        if (cancelled) return
        setAvailableSkills(
          skills.map((skill) => ({
            name: `/${skill.name}`,
            description: skill.description,
            skill: true,
          }))
        )
      })
      .catch(() => {
        if (!cancelled) setAvailableSkills([])
      })
    return () => {
      cancelled = true
    }
  }, [agentId])

  const filteredCommands = useMemo(() => {
    if (!input.startsWith('/')) return []
    const query = unescapeCommand(input).trim().toLowerCase()
    // Once the name is followed by input, the suggestions have done their job.
    if (/\s/.test(query)) return []
    return [...SLASH_COMMANDS, ...availableSkills].filter((cmd) =>
      cmd.name.toLowerCase().startsWith(query)
    )
  }, [input, availableSkills])

  // A skill goes into the composer, ready for its input, rather than being sent.
  const insertSuggestion = useCallback((cmd: SlashSuggestion) => {
    const value = cmd.skill ? `${cmd.name} ` : cmd.name
    setInput(value)
    currentInputRef.current = value
    setFocusComposer(true)
    setClearKey((k) => k + 1)
    setShowCommandSuggestions(false)
  }, [])

  const { addToast } = useToastStore()
  const {
    connected,
    lastMessage,
    // messageCount,
    sendMessage,
    saveClientState,
    // clearLastMessage,
  } = useConnectionStore()

  const [pageRef, { width: pageWidth }] = useMeasure()
  const [inputRef, { height: inputHeight }] = useMeasure()

  const resolvedTopicId = topicId ?? 'General'

  // Pulse send button when input transitions from empty to non-empty
  useEffect(() => {
    if (input.trim() && !prevInputRef.current.trim()) {
      setSendPulse(true)
      const timer = setTimeout(() => setSendPulse(false), 300)
      return () => clearTimeout(timer)
    }
    prevInputRef.current = input
  }, [input])

  // Show/hide command suggestions based on input
  useEffect(() => {
    if (input.startsWith('/') && input.length > 0) {
      setShowCommandSuggestions(true)
      setSelectedCommandIndex(0)
    } else {
      setShowCommandSuggestions(false)
    }
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
    if (!connected || !agentId || !topicId || !pendingMsgRef.current) return
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
      setThinking(true)

      sendMessage(message)
    }, 50)
    return () => clearTimeout(timer)
  }, [connected, agentId, topicId])

  const [topicDetail, setTopicDetail] = useState<any | null>(null)
  const [agentNames, setAgentNames] = useState<Record<string, string>>({})

  // Redirect to last topic (or General) if no topicId. Waits for the user: redirecting
  // before it loads lands on General, which the effect below would then save as the last
  // topic in place of the real one.
  useEffect(() => {
    if (agentId && !topicId && !userLoading) {
      const lastTopic = user?.user_id
        ? localStorage.getItem(`vizier_last_topic_${user.user_id}_${agentId}`)
        : null
      navigate(`/${agentId}/chat/${lastTopic || 'General'}`, { replace: true })
    }
  }, [agentId, topicId, navigate, user?.user_id, userLoading])

  // Persist current topic per agent
  useEffect(() => {
    if (agentId && topicId && user?.user_id) {
      localStorage.setItem(`vizier_last_topic_${user.user_id}_${agentId}`, topicId)
    }
  }, [agentId, topicId, user?.user_id])

  // Load topic detail. Nothing topic-scoped loads until the URL names a topic, so the
  // redirect above never fetches General on the way to another topic.
  useEffect(() => {
    if (!agentId || !topicId) return
    let stale = false
    listTopics(agentId).then((topic) => {
      if (stale) return
      const detail = topic.data.find(
        (item: any) => item.topic_id === topicId
      )
      setTopicDetail(detail || null)
    })
    return () => {
      stale = true
    }
  }, [agentId, topicId])

  // Poll for thinking state: catches turns this page saw no frames for. The socket is
  // fresher, so a poll issued within THINKING_POLL_GRACE_MS of the socket (or a send)
  // changing the state is ignored: the server sends a turn's reply before it clears
  // `is_thinking`, and marks it only once the turn starts.
  useEffect(() => {
    if (!agentId || !topicId) return
    let stale = false

    const pollThinkingState = async () => {
      const issuedAt = Date.now()
      let thinking: boolean
      try {
        const res = await getTopicDetail(agentId, topicId)
        thinking = res.data?.is_thinking ?? false
      } catch {
        return
      }
      if (stale || issuedAt < thinkingChangedAtRef.current + THINKING_POLL_GRACE_MS) return
      if (thinking === isThinkingRef.current) return
      isThinkingRef.current = thinking
      setIsThinking(thinking)
      if (!thinking) {
        // The turn ended and its reply never arrived here (the socket was down, or the
        // page was elsewhere): history has it.
        setLiveTrail([])
        setHistoryVersion((v) => v + 1)
      }
    }

    // Initial fetch
    pollThinkingState()

    // Poll every 1000ms
    const interval = setInterval(pollThinkingState, 1000)

    return () => {
      stale = true
      clearInterval(interval)
    }
  }, [agentId, topicId])

  // Clear inline events, attachments, and queued messages when topic changes
  useEffect(() => {
    currentTopicRef.current = resolvedTopicId
    isThinkingRef.current = false
    thinkingChangedAtRef.current = 0
    setIsThinking(false)
    setLiveTrail([])
    setClosedTrails({})
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

  // Background jobs launched from this topic. The socket reloads them on every (re)connect
  // too; this covers opening a topic whose socket is already up.
  useEffect(() => {
    if (!agentId || !topicId) return
    void useBackgroundJobStore.getState().load(agentId, topicId)
    setOpenPiece(null)
  }, [agentId, topicId])

  // A job moving to `reporting` is the report being handed to the agent: show its entry
  // now rather than on the next reload. The snapshot carries prompts and states only; the
  // answers' text arrives with history. Behind a running turn it waits in the queue, which
  // is where the server is holding it too.
  useEffect(() => {
    const job = lastAppliedJob?.job
    if (!job || job.state !== 'reporting' || !agentId) return
    const { agentId: jobAgent, topicId: jobTopic } = useBackgroundJobStore.getState()
    if (jobAgent !== agentId || jobTopic !== resolvedTopicId) return

    const report: BackgroundReport = {
      job_id: job.id,
      kind: job.kind,
      delegated_to: job.delegated_to,
      entries: job.pieces.map((piece) => ({
        ordinal: piece.ordinal,
        prompt: piece.prompt,
        state: piece.state,
        text: piece.state === 'answered' ? '' : piece.reason ?? '',
        truncated: false,
      })),
    }
    const uid = `bg-report-${job.id}`
    const entry: ChatMessage = {
      uid,
      timestamp: new Date().toISOString(),
      vizier_session: { agent_id: agentId, channel: 'vizier-webui', topic: resolvedTopicId },
      content: {
        Request: {
          timestamp: new Date().toISOString(),
          user: agentId,
          content: { background_report: report },
        },
      },
    }
    const seen = (list: ChatMessage[]) =>
      list.some(
        (m) =>
          m.uid === uid ||
          (m.content.Request?.content &&
            'background_report' in m.content.Request.content &&
            m.content.Request.content.background_report.job_id === job.id)
      )
    if (isThinkingRef.current) {
      setQueuedMessages((prev) => (seen(prev) ? prev : [...prev, entry]))
    } else {
      setMessages((prev) => (seen(prev) ? prev : [...prev, entry]))
    }
  }, [lastAppliedJob])

  // The open piece's conversation. Its own frames are not streamed, so while it runs it is
  // re-read whenever its job changes.
  const openPieceJob = lastAppliedJob?.job.id === openPiece?.jobId ? lastAppliedJob?.n : 0
  useEffect(() => {
    if (!openPiece || !agentId) return
    let stale = false
    getPieceHistory(agentId, resolvedTopicId, openPiece.jobId, openPiece.ordinal)
      .then((res) => {
        if (!stale) setPieceHistory(res.data || [])
      })
      .catch((err) => {
        console.error('Failed to load piece history', err)
        if (!stale) setPieceHistory([])
      })
    return () => {
      stale = true
    }
  }, [openPiece, openPieceJob, agentId, resolvedTopicId])

  const openPieceTurns = useMemo(() => groupHistory(pieceHistory ?? []), [pieceHistory])

  const handleOpenPiece = useCallback((jobId: string, ordinal: number) => {
    setPieceHistory(null)
    setOpenPiece({ jobId, ordinal })
  }, [])

  const closePiece = useCallback(() => setOpenPiece(null), [])

  // `↥ jump`: the tool call that launched the job carries its id (see `lib/trail.ts`).
  const handleJumpToJob = useCallback((jobId: string) => {
    const target = scrollContainerRef.current?.querySelector<HTMLElement>(
      `[data-job-id="${CSS.escape(jobId)}"]`
    )
    if (!target) {
      addToast('info', 'Not on screen', `The call that started ${jobId} is not in the loaded history.`)
      return
    }
    const details = target.closest('details')
    if (details) details.open = true
    target.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }, [])

  useEffect(() => {
    if (!agentId) return
    getAgentDetail(agentId).then((data) => {
      setAgentDetail(data.data)
    })
  }, [agentId])

  // Load chat history. A response for a topic this page has since left is dropped, so a
  // slow read can never paint one topic's messages under another.
  useEffect(() => {
    if (!agentId || !topicId) return
    let stale = false

    const key = `${agentId}/${topicId}`
    if (loadedTopicRef.current !== key) {
      // A new topic: the previous one's messages go now, not when this one arrives.
      loadedTopicRef.current = key
      setLoading(true)
      setMessages([])
      setReactions({})
    }

    const loadHistory = async () => {
      try {
        const response = await getTopicHistory(agentId, topicId)
        if (stale) return
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
        if (stale) return
        console.error('Failed to load chat history:', error)
        setMessages([])
      } finally {
        if (!stale) setLoading(false)
      }
    }

    loadHistory()
    return () => {
      stale = true
    }
  }, [agentId, topicId, historyVersion])

  useEffect(() => {
    listAgents().then((res) => {
      const names: Record<string, string> = {}
      res.data.forEach((agent: Agent) => {
        names[agent.agent_id] = agent.name
      })
      setAgentNames(names)
    })
  }, [])

  const clearLiveTrail = () => {
    setLiveTrail([])
    if (thinkingTimeoutRef.current) {
      clearTimeout(thinkingTimeoutRef.current)
      thinkingTimeoutRef.current = null
    }
  }

  /// Hands the closing turn the trail it accumulated, instead of throwing it away.
  const closeTurn = (anchorUid: string) => {
    setLiveTrail((trail) => {
      if (trail.length > 0) {
        setClosedTrails((prev) => ({ ...prev, [anchorUid]: trail }))
      }
      return []
    })
    if (thinkingTimeoutRef.current) {
      clearTimeout(thinkingTimeoutRef.current)
      thinkingTimeoutRef.current = null
    }
  }

  const toolLabel = useCallback(
    (name: string, args: Record<string, unknown>) => formatToolChoice(name, args, agentNames),
    [agentNames]
  )

  // Stored history, grouped into turns, so a trail recorded before this page loaded renders
  // in the same place and through the same component as one collected live.
  const turns = useMemo(() => groupHistory(messages), [messages])

  const storedTrails = useMemo(() => {
    const byAnchor: Record<string, { trail: TrailEvent[]; durationMs?: number }> = {}
    const unanchored: { trail: TrailEvent[]; key: string }[] = []
    for (const turn of turns) {
      if (turn.trail.length === 0) continue
      if (turn.anchorUid) {
        byAnchor[turn.anchorUid] = {
          trail: turn.trail,
          durationMs: outcomeDurationMs(turn.outcome),
        }
      } else {
        // A turn nothing closed — interrupted, or still running when the page loaded.
        unanchored.push({ trail: turn.trail, key: turn.key })
      }
    }
    return { byAnchor, unanchored }
  }, [turns])

  // Each turn's client tool calls, drawn inside its reply bubble: the answer's, keyed by the
  // answer's uid, or — for a turn with no answer yet, or none ever (running, failed before
  // replying, aborted) — a bubble of their own where the first call happened.
  const clientCalls = useMemo(() => {
    const byOutcome: Record<string, ClientCallView[]> = {}
    const atCall: Record<string, ClientCallView[]> = {}
    turns.forEach((turn, at) => {
      // Only the last turn can still be running; one with no outcome before it ended
      // without one (aborted, interrupted).
      const running = !turn.outcome && at === turns.length - 1 && isThinking
      const calls = visibleClientCalls(turn.clientCalls, running)
      if (calls.length === 0) return
      if (turn.outcome) byOutcome[turn.outcome.uid] = calls
      else atCall[calls[0].callId] = calls.map((call) => ({ ...call, live: running }))
    })
    return { byOutcome, atCall }
  }, [turns, isThinking])

  // A turn closed in this session wins: its events are the ones the person just watched.
  const trailFor = (uid: string) =>
    closedTrails[uid] !== undefined
      ? { trail: closedTrails[uid], durationMs: undefined }
      : storedTrails.byAnchor[uid]

  // Read through a ref so `handleReact` keeps one identity across reconnects: a message
  // rendered while offline must react normally once the socket is back (W5).
  const connectedRef = useRef(connected)
  useEffect(() => {
    connectedRef.current = connected
  }, [connected])
  const reactionsRef = useRef(reactions)
  useEffect(() => {
    reactionsRef.current = reactions
  }, [reactions])

  // `${uid}|${emoji}` sent and not yet answered. The displayed set changes only when the
  // server's `reactions` frame arrives, so nothing unsaved is ever shown as saved (W1–W3).
  const [pendingReactions, setPendingReactions] = useState<Set<string>>(() => new Set())
  const pendingTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())

  const clearPending = useCallback((predicate: (key: string) => boolean) => {
    for (const [key, timer] of pendingTimersRef.current) {
      if (predicate(key)) {
        clearTimeout(timer)
        pendingTimersRef.current.delete(key)
      }
    }
    setPendingReactions((prev) => {
      const next = new Set([...prev].filter((key) => !predicate(key)))
      return next.size === prev.size ? prev : next
    })
  }, [])

  useEffect(() => {
    const timers = pendingTimersRef.current
    return () => {
      timers.forEach(clearTimeout)
      timers.clear()
    }
  }, [])

  const handleReact = useCallback(
    (messageUid: string, emoji: string) => {
      if (!connectedRef.current) {
        addToast('error', 'Not connected', 'Reconnect to react to messages.')
        return
      }

      const key = `${messageUid}|${emoji}`
      if (pendingTimersRef.current.has(key)) return

      const currentUserId = getCurrentUsername()
      const mine = (reactionsRef.current[messageUid] || []).some(
        (r) => r.user_id === currentUserId && r.emoji === emoji
      )

      setPendingReactions((prev) => new Set(prev).add(key))
      pendingTimersRef.current.set(
        key,
        setTimeout(() => {
          if (!pendingTimersRef.current.has(key)) return
          clearPending((k) => k === key)
          addToast('error', 'Reaction not saved', 'The server did not confirm it in time.')
        }, 5000)
      )

      sendMessage({
        reaction: {
          message_uid: messageUid,
          emoji,
          action: mine ? 'removed' : 'added',
        },
      })
    },
    [sendMessage, addToast, clearPending]
  )

  // The server's answer to a reaction from any tab, any channel, or a moderator: the full
  // new set for one message, or this socket's own failure.
  useEffect(() => {
    return subscribeReactionFrames((frame) => {
      if ('reactions' in frame) {
        const { message_uid, reactions: set } = frame.reactions
        setReactions((prev) => ({ ...prev, [message_uid]: set }))
        clearPending((key) => key.startsWith(`${message_uid}|`))
      } else {
        const { message_uid, message } = frame.reaction_error
        clearPending((key) => key.startsWith(`${message_uid}|`))
        addToast('error', 'Reaction not saved', message)
      }
    })
  }, [addToast, clearPending])

  // Client tool calls as they happen: the call (this tab's own, or another tab's display
  // copy), how it ended, and state any tab saved for it.
  useEffect(() => {
    return subscribeClientFrames((frame) => {
      if ('client_tool_call' in frame) {
        const session = {
          agent_id: agentId!,
          channel: 'vizier-webui',
          topic: currentTopicRef.current!,
        }
        setMessages((prev) => appendClientCall(prev, frame.client_tool_call, session))
      } else if ('client_tool_outcome' in frame) {
        setMessages((prev) => applyClientOutcome(prev, frame.client_tool_outcome))
      } else if ('client_call_state' in frame) {
        setMessages((prev) => applyClientState(prev, frame.client_call_state))
      } else if ('client_error' in frame && frame.client_error.call_id) {
        const { code, message } = frame.client_error
        if (code === 'rate_limited' || code === 'state_too_large' || code === 'state_not_saved') {
          console.warn(`Client tool call ${frame.client_error.call_id}: ${message}`)
        }
      }
    })
  }, [agentId])

  const pendingByMessage = useMemo(() => {
    const byUid: Record<string, string[]> = {}
    for (const key of pendingReactions) {
      const cut = key.indexOf('|')
      const uid = key.slice(0, cut)
      ;(byUid[uid] ||= []).push(key.slice(cut + 1))
    }
    return byUid
  }, [pendingReactions])

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
        // A turn the person did not start — one a background report woke — announces
        // itself only through its frames.
        setThinking(true)
        return

      case 'empty':
        setThinking(false)
        clearLiveTrail()
        return

      case 'abort':
        // An aborted turn has no answer for a trail to belong to, so the trail goes.
        setThinking(false)
        clearLiveTrail()
        setQueuedMessages([])
        return
    }

    if (typeof content === 'object') {
      if ('thinking' in content || 'tool_choice' in content || 'tool_response' in content) {
        const id = `${timestamp}-${Math.random().toString(36).slice(2, 11)}`
        setLiveTrail((trail) => appendLiveEvent(trail, content, id))
        return
      }

      if ('checkpoint' in content) {
        // Add checkpoint to messages
        setThinking(false)
        closeTurn(`checkpoint-${timestamp}`)
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
        setThinking(false)
        const uid = wsResponse.history_uid ?? timestamp
        closeTurn(uid)
        setMessages((prev) => {
          if (
            prev.some(
              (m) => m.uid === uid || m.content.Response?.timestamp === timestamp
            )
          ) {
            return prev
          }
          const newMessage: ChatMessage = {
            uid,
            serverUid: wsResponse.history_uid !== undefined,
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
        setThinking(false)
        closeTurn(timestamp)
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
            serverUid: false,
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
        setThinking(false)
        const uid = wsResponse.history_uid ?? timestamp
        closeTurn(uid)
        setMessages((prev) => {
          if (
            prev.some(
              (m) => m.uid === uid || m.content.Response?.timestamp === timestamp
            )
          ) {
            return prev
          }
          const newMessage: ChatMessage = {
            uid,
            serverUid: wsResponse.history_uid !== undefined,
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
  }, [messages, liveTrail, queuedMessages, showScrollButton])

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

      // Handle slash commands
      const text = unescapeCommand(currentInput.trim())
      if (text === '/checkpoint' || text === '/lobotomy' || text === '/abort') {
        const command = text.slice(1) // Remove leading /
        const message: WebSocketMessage = {
          timestamp: new Date().toISOString(),
          user: username,
          content: { command },
          metadata: null as any,
        }

        // Add command to chat history
        const userMessage: ChatMessage = {
          uid: Date.now().toString(),
          vizier_session: {
            agent_id: agentId,
            channel: 'vizier-webui',
            topic: resolvedTopicId,
          },
          content: {
            Command: command,
          },
        }
        setMessages((prev) => [...prev, userMessage])

        sendMessage(message)
        setInput('')
        currentInputRef.current = ''
        setClearKey((k) => k + 1)
        setShowCommandSuggestions(false)
        setPlaceholderSeed(Math.random())
        return
      }

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
          content: { chat: unescapeCommand(currentInput.trim()) },
          metadata: null as any,
          attachments: uploadedAttachments,
          expect_audio_reply: expectAudioReply || undefined,
        }

        userMessageContent = {
          Request: {
            timestamp: new Date().toISOString(),
            user: username,
            content: { chat: unescapeCommand(currentInput.trim()) },
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
      if (isThinkingRef.current) {
        setQueuedMessages((prev) => [...prev, userMessage])
      } else {
        setMessages((prev) => [...prev, userMessage])
        setThinking(true)
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
    [agentId, resolvedTopicId, connected, sendMessage, attachments, addToast, setThinking, recordingState, recordedBlob, recordedUrl]
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

  // A page (or any client tool) starting a turn as the person, the way a typed message does.
  const handleClientEvent = useCallback(
    (text: string, data: unknown, sourceCallId: string) => {
      if (!agentId) return
      if (!connected) {
        addToast('error', 'Not connected', 'Reconnect to send from the page.')
        return
      }
      const message = clientEventMessage(text, data, sourceCallId)
      const userMessage: ChatMessage = {
        uid: Date.now().toString(),
        vizier_session: {
          agent_id: agentId,
          channel: 'vizier-webui',
          topic: resolvedTopicId,
        },
        content: {
          Request: {
            timestamp: message.timestamp,
            user: message.user,
            content: message.content,
          },
        },
      }
      if (isThinkingRef.current) {
        setQueuedMessages((prev) => [...prev, userMessage])
      } else {
        setMessages((prev) => [...prev, userMessage])
        setThinking(true)
      }
      sendMessage(message)
      // A page sends on the person's behalf, often while expanded over the conversation, so
      // say what went out.
      addToast('info', 'Sent from the page', text.length > 120 ? `${text.slice(0, 120)}…` : text)
    },
    [agentId, connected, resolvedTopicId, sendMessage, addToast, setThinking]
  )

  // The context a call's renderer is drawn with. Read through a ref so `renderClientCall`
  // keeps one identity and memoised bubbles do not redraw on every send.
  const clientCtxRef = useRef({ handleClientEvent, saveClientState, agentId, resolvedTopicId })
  clientCtxRef.current = { handleClientEvent, saveClientState, agentId, resolvedTopicId }
  const renderClientCall = useCallback((call: ClientCallView) => {
    const ctx: ClientCallContext = {
      callId: call.callId,
      agentId: clientCtxRef.current.agentId ?? '',
      topicId: clientCtxRef.current.resolvedTopicId,
      sendEvent: (text, data) => clientCtxRef.current.handleClientEvent(text, data, call.callId),
      saveState: (state) => clientCtxRef.current.saveClientState(call.callId, state),
    }
    const render = findClientTool(call.category, call.tool)?.render
    return render ? render(call, ctx) : <ClientCallChip call={call} />
  }, [])

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
                        {(session.running_jobs ?? 0) > 0 && (
                          <span
                            className="bg-topic-badge"
                            title={`${session.running_jobs} background job(s) running`}
                            style={{ marginLeft: 6 }}
                          >
                            ⟳{session.running_jobs}
                          </span>
                        )}
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
          {messages.length === 0 && liveTrail.length === 0 ? (
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
                // The trail of the turn this entry closes. An answer carries its own
                // inside its bubble (see the MessageItem below); a divider has no bubble to
                // sit in, so for those it is drawn as a block of its own.
                const own = trailFor(msg.uid)
                const trail = own ? (
                  <ActivityTrail
                    key={`${msg.uid}-trail`}
                    trail={own.trail}
                    live={false}
                    durationMs={own.durationMs}
                    label={toolLabel}
                  />
                ) : null

                // Handle checkpoint entries
                if (msg.content.Checkpoint !== undefined) {
                  const cp = msg.content.Checkpoint
                  const handover = typeof cp === 'string' ? cp : cp?.handover ?? null
                  const timestamp = typeof cp === 'string'
                    ? (msg.timestamp || new Date().toISOString())
                    : cp?.timestamp || msg.timestamp || new Date().toISOString()
                  return (
                    <Fragment key={msg.uid}>
                      {trail}
                      <CheckpointDivider handover={handover} timestamp={timestamp} />
                    </Fragment>
                  )
                }

                // Handle command entries
                if (msg.content.Command) {
                  const cancelledJob = msg.content.Command.startsWith('cancelled background job ')
                    ? msg.content.Command.slice('cancelled background job '.length)
                    : null
                  return (
                    <Fragment key={msg.uid}>
                      {trail}
                      <div className="command-history-entry">
                        <span className="command-history-icon">{cancelledJob ? '⊘' : '⚡'}</span>
                        <span className="command-history-text">
                          {cancelledJob
                            ? `You cancelled background job ${cancelledJob}`
                            : `/${msg.content.Command}`}
                        </span>
                      </div>
                    </Fragment>
                  )
                }

                // A client tool call renders inside its turn's reply. Only a turn with no
                // reply has its calls drawn here, at the first call, in a bubble of their own.
                if (msg.content.ClientToolCall) {
                  const calls = clientCalls.atCall[msg.uid]
                  if (!calls) return trail
                  return (
                    <Fragment key={msg.uid}>
                      {trail}
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                        <div style={{ fontWeight: '600', fontSize: '14px', color: 'var(--accent-primary)' }}>
                          {agentDetail?.name || 'Agent'}
                        </div>
                        <div
                          style={{
                            padding: '12px 16px',
                            borderRadius: '8px',
                            borderLeft: '3px solid var(--accent-primary)',
                            display: 'flex',
                            flexDirection: 'column',
                            gap: '8px',
                          }}
                        >
                          {calls.map((call) => (
                            <div key={call.callId}>{renderClientCall(call)}</div>
                          ))}
                        </div>
                      </div>
                    </Fragment>
                  )
                }

                // A background report woke the agent; it is not a person's message.
                const requestContent = msg.content.Request?.content
                if (requestContent && 'background_report' in requestContent) {
                  return (
                    <Fragment key={msg.uid}>
                      {trail}
                      <BackgroundReportItem
                        report={requestContent.background_report}
                        onOpenPiece={handleOpenPiece}
                      />
                    </Fragment>
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
                let senderNote: string | undefined

                if (isUserMessage && msg.content.Request) {
                  const request = msg.content.Request as any
                  if (request.content?.chat) {
                    content = request.content.chat
                  } else if (request.content?.client_event) {
                    // The person's click in a page: its text is their message; its data is
                    // for the agent and never shown.
                    content = request.content.client_event.text
                    senderNote = 'from a page'
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
                                    errorContent.kind === 'rate_limited' ? 'Rate Limited' :
                                    errorContent.kind === 'provider_unavailable' ? 'Provider Unavailable' :
                                    errorContent.kind === 'agent_unavailable' ? 'Agent Unavailable' :
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

                // Narration, tool calls and tool results are part of a trail, not messages
                // of their own, so they render nothing at this level.
                if (!content) return trail

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
                    pendingEmojis={pendingByMessage[msg.uid]}
                    currentUserId={getCurrentUsername()}
                    canReact={!isUserMessage && !isError && msg.serverUid !== false}
                    onReact={handleReact}
                    onCopy={handleCopyMessage}
                    onPreviewAttachment={setPreviewAttachment}
                    isVoiceMessage={isVoiceMessage}
                    voiceSrc={voiceSrc}
                    audioReplySrc={audioReplySrc}
                    isError={isError}
                    trail={isUserMessage ? undefined : own?.trail}
                    trailDurationMs={own?.durationMs}
                    trailLabel={toolLabel}
                    senderNote={senderNote}
                    clientCalls={isUserMessage ? undefined : clientCalls.byOutcome[msg.uid]}
                    renderClientCall={renderClientCall}
                  />
                )
              })}

              {/* A turn nothing closed — interrupted, or still running when the page
                  loaded. Its trail belongs to no entry, so it renders where it happened. */}
              {storedTrails.unanchored.map((item) => (
                <ActivityTrail
                  key={`${item.key}-trail`}
                  trail={item.trail}
                  live={false}
                  label={toolLabel}
                />
              ))}

              {/* Thinking indicator with inline events */}
              <ThinkingIndicator
                isVisible={isThinking}
                trail={liveTrail}
                agentName={agentDetail?.name || 'Agent'}
                toolLabel={toolLabel}
                onAbort={isThinking ? handleAbort : undefined}
              />

              {/* Queued messages */}
              {queuedMessages.map((msg) => {
                const queuedContent = msg.content.Request?.content
                if (queuedContent && 'background_report' in queuedContent) {
                  return (
                    <BackgroundReportItem
                      key={msg.uid}
                      report={queuedContent.background_report}
                      onOpenPiece={handleOpenPiece}
                      queued
                    />
                  )
                }
                const request = msg.content.Request as any
                const content = request?.content?.chat || request?.content?.client_event?.text || (request?.content?.audio_chat ? (request.content.audio_chat[1] || '🎤 Voice message') : null)
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
            {agentId && (
              <BackgroundJobTray
                agentId={agentId}
                topicId={resolvedTopicId}
                onJump={handleJumpToJob}
                onOpenPiece={handleOpenPiece}
              />
            )}
            {/* Command suggestions */}
            {showCommandSuggestions && filteredCommands.length > 0 && (
              <div className="command-suggestions">
                {filteredCommands.map((cmd, index) => (
                  <div
                    key={cmd.name}
                    className={`command-suggestion-item ${index === selectedCommandIndex ? 'selected' : ''}`}
                    onClick={() => {
                      if (cmd.skill || cmd.name === '/skills') {
                        insertSuggestion(cmd)
                        return
                      }
                      const command = cmd.name.slice(1) // Remove leading /
                      const message: WebSocketMessage = {
                        timestamp: new Date().toISOString(),
                        user: getCurrentUsername(),
                        content: { command },
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
                          Command: command,
                        },
                      }
                      setMessages((prev) => [...prev, userMessage])
                      sendMessage(message)
                      setInput('')
                      currentInputRef.current = ''
                      setClearKey((k) => k + 1)
                      setShowCommandSuggestions(false)
                      setPlaceholderSeed(Math.random())
                    }}
                    onMouseEnter={() => setSelectedCommandIndex(index)}
                  >
                    <span className="command-suggestion-name">{cmd.name}</span>
                    <span className="command-suggestion-description">{cmd.description}</span>
                  </div>
                ))}
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
                // Command suggestions keyboard navigation
                if (showCommandSuggestions && filteredCommands.length > 0) {
                  if (e.key === 'ArrowDown') {
                    e.preventDefault()
                    setSelectedCommandIndex((prev) =>
                      prev < filteredCommands.length - 1 ? prev + 1 : 0
                    )
                  } else if (e.key === 'ArrowUp') {
                    e.preventDefault()
                    setSelectedCommandIndex((prev) =>
                      prev > 0 ? prev - 1 : filteredCommands.length - 1
                    )
                  } else if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
                    e.preventDefault()
                    const selected = filteredCommands[selectedCommandIndex]
                    if (selected) insertSuggestion(selected)
                  } else if (e.key === 'Escape') {
                    setShowCommandSuggestions(false)
                  }
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
                    autoFocus={focusComposer}
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

      {/* A background piece's own conversation, read-only. */}
      <SlideOver
        open={openPiece !== null}
        onClose={closePiece}
        title={openPiece ? `${openPiece.jobId} · piece ${openPiece.ordinal + 1}` : ''}
      >
        {pieceHistory === null ? (
          <SkeletonMessage />
        ) : openPieceTurns.length === 0 ? (
          <p style={{ color: 'var(--text-tertiary)' }}>Nothing recorded for this piece yet.</p>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            {openPieceTurns.map((turn) => {
              const prompt = turn.request?.content.Request?.content
              const promptText =
                prompt && 'prompt' in prompt
                  ? prompt.prompt
                  : prompt && 'chat' in prompt
                    ? prompt.chat
                    : prompt && 'client_event' in prompt
                      ? prompt.client_event.text
                      : null
              const outcome = turn.outcome?.content.Response?.content
              const answer =
                typeof outcome === 'object' && outcome !== null
                  ? 'message' in outcome
                    ? outcome.message.content
                    : 'error' in outcome
                      ? `**Error**: ${outcome.error.message}`
                      : null
                  : outcome === 'abort'
                    ? '_Aborted._'
                    : null
              return (
                <div key={turn.key} style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  {promptText && (
                    <div className="bg-report-entry" style={{ whiteSpace: 'pre-wrap' }}>
                      {promptText}
                    </div>
                  )}
                  <ActivityTrail
                    trail={turn.trail}
                    live={false}
                    durationMs={outcomeDurationMs(turn.outcome)}
                    label={toolLabel}
                  />
                  {answer && (
                    <div className="prose">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{answer}</ReactMarkdown>
                    </div>
                  )}
                </div>
              )
            })}
            {(() => {
              const live = openPiece
                ? useBackgroundJobStore.getState().jobs.get(openPiece.jobId)?.pieces[openPiece.ordinal]
                : undefined
              return live?.state === 'running' ? (
                <p style={{ color: 'var(--text-tertiary)' }}>
                  <span className="bg-spin">⟳</span> Still running…
                </p>
              ) : null
            })()}
          </div>
        )}
      </SlideOver>
    </>
  )
}
