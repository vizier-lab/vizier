import { IconButton, JobsPill, SlideOver } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { cancelJob, listTopics, restartAgent } from '../api/endpoints/chat'
import type { Topic, Turn } from '../api/frames'
import type { ClientCallView } from '../api/frames'
import { emptyTopic } from '../lib/frames'
import { requestText } from '../lib/messages'
import { ConversationHeader } from '../screens/chat/ConversationHeader'
import { AgentStateBanner } from '../screens/chat/AgentStateBanner'
import { BackgroundJobsPanel } from '../screens/chat/BackgroundJobsPanel'
import { renderClientCalls } from '../screens/chat/ClientCalls'
import { Composer } from '../screens/chat/Composer'
import { ErrorNotice } from '../screens/chat/ErrorNotice'
import { MessageList } from '../screens/chat/MessageList'
import {
    RecalledMemoryPanel,
    recalledFrom,
} from '../screens/chat/RecalledMemoryPanel'
import { SessionFilesPanel } from '../screens/chat/SessionFilesPanel'
import { useMobile } from '../lib/useMobile'
import { useAgents } from '../stores/agents'
import { topicKey, useConnection } from '../stores/connection'
import { toast } from '../stores/toasts'
import { useTurns } from '../stores/turns'

const EMPTY = emptyTopic()

export default function Chat() {
    const { agentId = '', topicId } = useParams()
    const navigate = useNavigate()
    const agents = useAgents((s) => s.agents)
    const health = useAgents((s) => s.health[agentId])
    const setup = useAgents((s) => s.setups[agentId])
    const samples = useAgents((s) => s.samples[agentId])
    const agent = agents.find((a) => a.agent_id === agentId)
    const agentName = agent?.name ?? agentId

    const [topics, setTopics] = useState<Topic[]>([])
    const mobile = useMobile()
    const [panel, setPanel] = useState(true)
    const [sheet, setSheet] = useState(false)

    // No topic in the URL: the latest one, or a fresh one.
    useEffect(() => {
        let alive = true
        listTopics(agentId)
            .then((list) => {
                if (!alive) return
                setTopics(list ?? [])
                if (!topicId)
                    navigate(
                        `/${agentId}/chat/${list?.[0]?.topic_id ?? nanoid(10)}`,
                        { replace: true }
                    )
            })
            .catch(() => {
                if (alive && !topicId)
                    navigate(`/${agentId}/chat/${nanoid(10)}`, {
                        replace: true,
                    })
            })
        return () => {
            alive = false
        }
    }, [agentId, topicId === undefined]) // eslint-disable-line react-hooks/exhaustive-deps

    const open = useTurns((s) => s.open)
    const close = useTurns((s) => s.close)
    useEffect(() => {
        if (!topicId) return
        void open(agentId, topicId)
        return () => close(agentId, topicId)
    }, [agentId, topicId, open, close])

    const key = topicKey(agentId, topicId ?? '')
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
    const title = topics[topicIndex]?.title || topicId || 'New topic'

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
        <div data-chat-area className="flex min-h-0 flex-1">
            <div className="flex min-w-0 flex-1 flex-col">
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
                        state: failed
                            ? health?.next_retry_at
                                ? 'retrying'
                                : 'failed'
                            : state.phase !== 'idle'
                              ? 'live'
                              : 'idle',
                        detail: state.phase !== 'idle' ? 'replying' : undefined,
                    }}
                    mobileTrailing={
                        <>
                            <JobsPill
                                compact
                                count={runningJobs}
                                onClick={() => setSheet(true)}
                            />
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
                                onClick={() => setPanel(true)}
                                open={panel}
                            />
                            <button
                                type="button"
                                className="type-small text-muted hover:text-fg"
                                onClick={() => setPanel((p) => !p)}
                                aria-pressed={panel}
                            >
                                {panel ? 'Hide panel' : 'Show panel'}
                            </button>
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

                <MessageList
                    state={state}
                    agent={{ id: agentId, name: agentName }}
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
                    jobHref={(a) => `/${a}/chat`}
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

                <div className="px-24 pb-16">
                    <Composer
                        agentId={agentId}
                        agentName={agentName}
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

            {!mobile && panel && (
                <aside
                    aria-label="Conversation details"
                    className="flex w-320 shrink-0 flex-col gap-24 overflow-y-auto border-l border-border p-16"
                >
                    {details}
                </aside>
            )}
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
