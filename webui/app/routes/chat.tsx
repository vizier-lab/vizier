import { IconButton, JobsPill, Presence, SlideOver, motionDurations, motionEases } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useEffect, useMemo, useRef, useState } from 'react'
import { motion, useReducedMotion } from 'motion/react'
import { useNavigate, useParams } from 'react-router'
import { agentState, getAgent } from '../api/endpoints/agents'
import { cancelJob, listTopics, restartAgent } from '../api/endpoints/chat'
import type { Topic, Turn } from '../api/frames'
import type { ClientCallView } from '../api/frames'
import { emptyTopic } from '../lib/frames'
import { greeting } from '../lib/greetings'
import { requestText } from '../lib/messages'
import { ConversationHeader } from '../screens/chat/ConversationHeader'
import { AgentStateBanner } from '../screens/chat/AgentStateBanner'
import { BackgroundJobsPanel } from '../screens/chat/BackgroundJobsPanel'
import { renderClientCalls } from '../screens/chat/ClientCalls'
import { Composer } from '../screens/chat/Composer'
import { ErrorNotice } from '../screens/chat/ErrorNotice'
import { MessageList } from '../screens/chat/MessageList'
import { recalledFrom } from '../lib/recall'
import { RecalledMemoryPanel } from '../screens/chat/RecalledMemoryPanel'
import { SessionFilesPanel } from '../screens/chat/SessionFilesPanel'
import { lastTopic, read, rememberTopic, write } from '../lib/remember'
import { useMobile } from '../lib/useMobile'
import { useAgents } from '../stores/agents'
import { topicKey, useConnection } from '../stores/connection'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'
import { useTurns } from '../stores/turns'

const EMPTY = emptyTopic()
// A new key so the details panel starts collapsed for everyone, including people
// who left it open under the old default.
const PANEL_KEY = 'vizier.chat.panel.v2'

export default function Chat() {
    const { agentId = '', topicId } = useParams()
    const navigate = useNavigate()
    const agents = useAgents((s) => s.agents)
    const health = useAgents((s) => s.health[agentId])
    const setup = useAgents((s) => s.setups[agentId])
    const samples = useAgents((s) => s.samples[agentId])
    const agent = agents.find((a) => a.agent_id === agentId)
    const agentName = agent?.name ?? agentId

    // The mic needs the agent's Speech-to-Text and "Ask for a spoken reply" its Text-to-Speech;
    // each is offered only once the agent is known to have it.
    const [voice, setVoice] = useState<{ stt: boolean; tts: boolean }>()
    useEffect(() => {
        let alive = true
        setVoice(undefined)
        getAgent(agentId)
            .then((detail) => {
                if (alive) setVoice({ stt: detail.stt, tts: detail.tts })
            })
            .catch(() => {
                /* offer both: the config read is not worth an error on the composer */
                if (alive) setVoice({ stt: true, tts: true })
            })
        return () => {
            alive = false
        }
    }, [agentId])

    const [topics, setTopics] = useState<Topic[]>([])
    const mobile = useMobile()
    const userId = useSession((s) => s.user?.user_id)
    const [panel, setPanel] = useState(() => read(PANEL_KEY, false))
    const togglePanel = (open: boolean) => {
        write(PANEL_KEY, open)
        setPanel(open)
    }
    const composerBox = useRef<HTMLDivElement>(null)
    const [composerHeight, setComposerHeight] = useState(120)
    // A person who asked for less movement gets the panel, not the choreography.
    const reducedMotion = useReducedMotion() ?? false
    useEffect(() => {
        const el = composerBox.current
        if (!el) return
        const ro = new ResizeObserver(() => setComposerHeight(el.offsetHeight))
        ro.observe(el)
        return () => ro.disconnect()
    }, [])
    const [sheet, setSheet] = useState(false)

    // No topic in the URL: the one this person last had open, else the latest, else a fresh one.
    useEffect(() => {
        let alive = true
        const remembered = userId ? lastTopic(userId, agentId) : null
        if (!topicId && remembered)
            navigate(`/${agentId}/chat/${remembered}`, { replace: true })
        listTopics(agentId)
            .then((list) => {
                if (!alive) return
                setTopics(list ?? [])
                if (!topicId && !remembered)
                    navigate(
                        `/${agentId}/chat/${list?.[0]?.topic_id ?? nanoid(10)}`,
                        { replace: true }
                    )
            })
            .catch(() => {
                if (alive && !topicId && !remembered)
                    navigate(`/${agentId}/chat/${nanoid(10)}`, {
                        replace: true,
                    })
            })
        return () => {
            alive = false
        }
    }, [agentId, topicId === undefined]) // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
        if (userId && topicId) rememberTopic(userId, agentId, topicId)
    }, [userId, agentId, topicId])

    const open = useTurns((s) => s.open)
    const close = useTurns((s) => s.close)
    useEffect(() => {
        if (!topicId) return
        void open(agentId, topicId)
        return () => close(agentId, topicId)
    }, [agentId, topicId, open, close])

    const key = topicKey(agentId, topicId ?? '')
    const opening = useMemo(() => greeting(), [key])
    const state = useTurns((s) => s.topics[key]) ?? EMPTY
    const meta = useTurns((s) => s.meta[key])
    const connection = useConnection((s) => s.state[key]) ?? 'connecting'
    const {
        send,
        sendAudio,
        command,
        loadOlder,
        react,
        clientEvent,
        saveClientState,
    } = useTurns.getState()

    // "Back online" shows for a moment after a drop.
    const wasDown = useRef(false)
    const [reconnected, setReconnected] = useState(false)
    useEffect(() => {
        if (connection === 'reconnecting' || connection === 'offline')
            wasDown.current = true
        else if (connection === 'open' && wasDown.current) {
            wasDown.current = false
            setReconnected(true)
            const t = setTimeout(() => setReconnected(false), 3000)
            return () => clearTimeout(t)
        }
    }, [connection])

    const recalled = useMemo(() => recalledFrom(state.entries), [state.entries])
    const jobs = useMemo(() => Object.values(state.jobs), [state.jobs])
    const runningJobs = jobs.filter(
        (j) => j.state === 'running' || j.state === 'reporting'
    ).length

    const lastRequestText = () => {
        for (let i = state.entries.length - 1; i >= 0; i--) {
            const r = state.entries[i].content.Request
            if (r && !('unattended' in r.content)) return requestText(r.content)
        }
        return ''
    }

    const clientContext = (call: ClientCallView) => ({
        callId: call.callId,
        agentId,
        topicId: topicId ?? '',
        sendEvent: (text: string, data?: unknown) =>
            clientEvent(agentId, topicId ?? '', text, data, call.callId),
        saveState: (s: unknown) =>
            saveClientState(agentId, topicId ?? '', call.callId, s),
    })

    const modelName = health?.model?.model
    const failed = health?.state === 'failed'
    const topicIndex = topics.findIndex((t) => t.topic_id === topicId)
    const title = state.title || topics[topicIndex]?.title || topicId || 'New topic'

    // The same fresh topic the picker's "New conversation" opens, one click from the header.
    const newConversation = (
        <IconButton
            icon="plus"
            aria-label="New conversation"
            onClick={() => navigate(`/${agentId}/chat/${nanoid(10)}`)}
        />
    )

    // The recalled memory, background jobs and session files: a side panel, or a sheet on a phone.
    const details = (
        <>
            <RecalledMemoryPanel agentId={agentId} recalled={recalled} />
            <BackgroundJobsPanel
                jobs={jobs}
                onCancel={(id) =>
                    void cancelJob(agentId, topicId ?? '', id).catch((e) =>
                        toast.error("Couldn't cancel the job", e.message)
                    )
                }
                onJumpToStart={() => {}}
            />
            <SessionFilesPanel files={state.files} />
        </>
    )

    return (
        <div data-chat-area className="flex min-h-0 flex-1 flex-col">
            <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                <ConversationHeader
                    agentId={agentId}
                    title={title}
                    meta={
                        topicIndex >= 0
                            ? `webui · topic ${topicIndex + 1} of ${topics.length}`
                            : 'webui · new topic'
                    }
                    current={{ topic: topicId }}
                    agentName={agentName}
                    status={{
                        // This topic's turn, not any of the agent's, decides running.
                        state:
                            !failed && state.phase !== 'idle'
                                ? 'running'
                                : agentState(health && { ...health, live: false }),
                        detail: state.phase !== 'idle' ? 'replying' : undefined,
                    }}
                    mobileTrailing={
                        <>
                            <JobsPill
                                compact
                                count={runningJobs}
                                onClick={() => setSheet(true)}
                            />
                            {newConversation}
                            <IconButton
                                icon="layers"
                                aria-label="Conversation details"
                                onClick={() => setSheet(true)}
                            />
                        </>
                    }
                    liveTopics={topics
                        .filter((t) => t.is_thinking)
                        .map((t) => t.topic_id)}
                    trailing={
                        <>
                            <JobsPill
                                count={runningJobs}
                                onClick={() => togglePanel(true)}
                                open={panel}
                            />
                            {newConversation}
                            <IconButton
                                icon="layers"
                                aria-label={panel ? 'Hide conversation details' : 'Show conversation details'}
                                aria-pressed={panel}
                                onClick={() => togglePanel(!panel)}
                            />
                        </>
                    }
                />

                <AgentStateBanner
                    agentName={agentName}
                    health={health}
                    setup={setup}
                    samples={samples}
                    connection={connection}
                    reconnected={reconnected}
                    queued={state.queued.length}
                    onRetryNow={() =>
                        void restartAgent(agentId).catch((e) =>
                            toast.error("Couldn't restart the agent", e.message)
                        )
                    }
                    onReconnect={() =>
                        useConnection.getState().retry(agentId, topicId ?? '')
                    }
                    configHref={`/${agentId}/config`}
                />

                <div className="flex min-h-0 flex-1">
                <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
                <MessageList
                    bottomInset={composerHeight}
                    state={state}
                    agent={{ id: agentId, name: agentName, live: ['live', 'running'].includes(agentState(health)) }}
                    greeting={opening}
                    hasOlder={!!meta?.loaded && !meta.exhausted}
                    loadingOlder={!!meta?.loadingOlder}
                    onLoadOlder={() => void loadOlder(agentId, topicId ?? '')}
                    onReact={(uid, emoji, action) =>
                        react(agentId, topicId ?? '', uid, emoji, action)
                    }
                    clientCalls={(turn: Turn, running) =>
                        renderClientCalls(turn, running, clientContext)
                    }
                    recalledFor={() => recalled.length}
                    error={
                        state.error && (
                            <ErrorNotice
                                kind={state.error.kind}
                                message={state.error.message}
                                agentName={agentName}
                                onRetry={() => {
                                    const text = lastRequestText()
                                    if (text) send(agentId, topicId ?? '', text)
                                }}
                                configHref={`/${agentId}/config`}
                                providersHref="/settings/providers"
                                healthHref={`/${agentId}/config`}
                            />
                        )
                    }
                />

                {/* The composer floats over the messages: its box is the frosted glass, over a fade to the page colour so the toggle below stays readable. */}
                <div ref={composerBox} className="pointer-events-none absolute inset-x-0 bottom-0 z-10">
                    <div aria-hidden className="absolute inset-0 bg-gradient-to-t from-panel via-panel/80 to-transparent" />
                    <div className="pointer-events-auto relative mx-auto w-full max-w-4xl px-24 pt-24 pb-16">
                    <Composer
                        agentId={agentId}
                        agentName={agentName}
                        sttEnabled={voice?.stt ?? false}
                        ttsEnabled={voice?.tts ?? false}
                        busy={state.phase !== 'idle'}
                        disabledReason={
                            failed
                                ? `${agentName} can't answer until it restarts`
                                : connection === 'offline'
                                  ? "You're offline"
                                  : undefined
                        }
                        context={
                            state.context
                                ? { ...state.context, detail: modelName }
                                : null
                        }
                        onSend={(text, attachments, spoken) =>
                            send(
                                agentId,
                                topicId ?? '',
                                text,
                                attachments,
                                spoken
                            )
                        }
                        onSendAudio={(attachment) =>
                            sendAudio(agentId, topicId ?? '', attachment)
                        }
                        onCommand={(c) => command(agentId, topicId ?? '', c)}
                        onStop={() => command(agentId, topicId ?? '', 'abort')}
                    />
                    </div>
                </div>
                </div>
                <Presence>
                    {!mobile && panel && (
                        <motion.aside
                            key="details"
                            aria-label="Conversation details"
                            className="flex w-320 shrink-0 flex-col gap-24 overflow-x-hidden overflow-y-auto border-l border-border p-16 [&>*]:min-w-0"
                            // The conversation makes room for the panel instead of
                            // jumping when it appears.
                            initial={{ width: reducedMotion ? 320 : 0, opacity: reducedMotion ? 1 : 0 }}
                            animate={{ width: 320, opacity: 1 }}
                            exit={reducedMotion ? { opacity: 0 } : { width: 0, opacity: 0 }}
                            transition={{ duration: motionDurations.slow, ease: motionEases.standard }}
                        >
                            {details}
                        </motion.aside>
                    )}
                </Presence>
                </div>
            </div>
            {mobile && (
                <SlideOver
                    open={sheet}
                    title="Conversation details"
                    onClose={() => setSheet(false)}
                >
                    {details}
                </SlideOver>
            )}
        </div>
    )
}
