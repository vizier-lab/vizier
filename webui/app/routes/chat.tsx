import { DeleteConversation, type DeleteTarget } from '../screens/chat/DeleteConversation'
import { DeleteWorktreeDialog } from '../screens/projects/DeleteWorktreeDialog'
import { FadeRule, SIDE_PANEL_WIDTH, SideCard } from '../screens/shared/SideCard'
import { IconButton, JobsPill, Notice, Presence, SlideOver, motionDurations, motionEases } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { motion, useReducedMotion } from 'motion/react'
import { useNavigate, useParams } from 'react-router'
import { agentState, getAgent } from '../api/endpoints/agents'
import { CHANNEL, cancelJob, listTopics, restartAgent } from '../api/endpoints/chat'
import { createProjectSession, projectChannel } from '../api/endpoints/projects'
import type { Topic, Turn } from '../api/frames'
import type { ClientCallView } from '../api/frames'
import { emptyTopic } from '../lib/frames'
import { greeting } from '../lib/greetings'
import { requestText } from '../lib/messages'
import { ConversationHeader } from '../screens/chat/ConversationHeader'
import { BranchChip } from '../screens/chat/BranchChip'
import { ChangesPanel } from '../screens/chat/ChangesPanel'
import { AgentStateBanner } from '../screens/chat/AgentStateBanner'
import { BackgroundJobsPanel } from '../screens/chat/BackgroundJobsPanel'
import { renderClientCalls } from '../screens/chat/ClientCalls'
import { renderQuestions } from '../screens/chat/Questions'
import { SensitiveAskDialog } from '../components/questions/SensitiveAskDialog'
import { Composer } from '../screens/chat/Composer'
import { SideChatPanel } from '../screens/chat/SideChatPanel'
import { ErrorNotice } from '../screens/chat/ErrorNotice'
import { MessageList } from '../screens/chat/MessageList'
import { recalledFrom } from '../lib/recall'
import { RecalledMemoryPanel } from '../screens/chat/RecalledMemoryPanel'
import { SessionFilesPanel } from '../screens/chat/SessionFilesPanel'
import { HostExecutionNotice } from '../screens/projects/HostExecutionNotice'
import { OpenProjectButton, ProjectsRefused } from '../screens/projects/ProjectsSection'
import { lastTopic, read, rememberTopic, write } from '../lib/remember'
import { useMobile } from '../lib/useMobile'
import { useAgents } from '../stores/agents'
import { topicKey, useConnection } from '../stores/connection'
import { useSession } from '../stores/session'
import { NO_PROJECTS, projectKey, useProjects } from '../stores/projects'
import { toast } from '../stores/toasts'
import { canManageArtifacts, canUseProjects } from '../lib/access'
import { useTurns } from '../stores/turns'
import { conversationTitle } from '../lib/conversations'

const EMPTY = emptyTopic()
// A new key so the details panel starts collapsed for everyone, including people
// who left it open under the old default.
const PANEL_KEY = 'vizier.chat.panel.v2'

export default function Chat() {
    const { agentId = '', topicId, projectId } = useParams()
    const navigate = useNavigate()
    // A project session is the same screen on the project's own channel (spec 040).
    const channel = projectId ? projectChannel(projectId) : CHANNEL
    const project = useProjects((s) =>
        projectId ? (s.byAgent[agentId] ?? []).find((p) => p.id === projectId) : undefined
    )
    const agentProjects = useProjects((s) => s.byAgent[agentId] ?? NO_PROJECTS)
    const refreshProjects = useProjects((s) => s.refresh)
    const insertSession = useProjects((s) => s.insertSession)
    const loadProjectSessions = useProjects((s) => s.loadSessions)
    const setProjectMode = useProjects((s) => s.setMode)
    const [buildingPlan, setBuildingPlan] = useState(false)
    // A project chat's title as the project list has it, for a chat the topics list hasn't seen yet.
    const projectSessionTitle = useProjects((s) =>
        projectId && topicId
            ? (s.sessions[projectKey(agentId, projectId)] ?? []).find((x) => x.topic === topicId)?.title
            : undefined
    )
    // The worktree this chat lives in, if any: deleting the chat then deletes the worktree too.
    const chatWorktree = useProjects((s) =>
        projectId && topicId
            ? ((s.sessions[projectKey(agentId, projectId)] ?? []).find((x) => x.topic === topicId)
                  ?.worktree ?? null)
            : null
    )
    const gitAvailable = useProjects((s) =>
        projectId && topicId
            ? !!s.git[`${projectKey(agentId, projectId)}/${topicId}`]?.available
            : false
    )
    const refreshGit = useProjects((s) => s.refreshGit)
    const projectMode = useProjects((s) =>
        projectId && topicId
            ? (s.sessions[projectKey(agentId, projectId)] ?? []).find((x) => x.topic === topicId)?.mode
            : undefined
    )
    useEffect(() => {
        if (projectId && !project) void refreshProjects(agentId)
    }, [agentId, projectId, project, refreshProjects])
    // The chat's saved mode and git header come from the project endpoints, per chat.
    useEffect(() => {
        if (!projectId) return
        void loadProjectSessions(agentId, projectId)
        if (topicId) void refreshGit(agentId, projectId, topicId)
    }, [agentId, projectId, topicId, loadProjectSessions, refreshGit])
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
    // Stable, so the panel's Escape listener is not re-bound on every render.
    const closeSideChat = useCallback(() => setSideChat((s) => (s.open ? { ...s, open: false } : s)), [])
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
    // `/btw` panel. The id changes per invocation, so a second /btw while it is open starts fresh.
    const [sideChat, setSideChat] = useState<{ open: boolean; id: string; question?: string }>({ open: false, id: '' })
    // A side chat belongs to the conversation it was opened on.
    useEffect(() => setSideChat((s) => (s.open ? { ...s, open: false } : s)), [agentId, topicId])

    // No topic in the URL: the one this person last had open, else the latest, else a fresh one.
    useEffect(() => {
        let alive = true
        const remembered = userId ? lastTopic(userId, agentId) : null
        if (!topicId && remembered)
            navigate(`/${agentId}/chat/${remembered}`, { replace: true })
        listTopics(agentId, channel)
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
    }, [agentId, channel, topicId === undefined]) // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
        if (userId && topicId) rememberTopic(userId, agentId, topicId)
    }, [userId, agentId, topicId])

    const open = useTurns((s) => s.open)
    const close = useTurns((s) => s.close)
    useEffect(() => {
        if (!topicId) return
        void open(agentId, topicId, channel)
        return () => close(agentId, topicId)
    }, [agentId, topicId, channel, open, close])

    const key = topicKey(agentId, topicId ?? '')
    const opening = useMemo(() => greeting(), [key])
    const state = useTurns((s) => s.topics[key]) ?? EMPTY
    // A finished turn may have changed files, so the header's count is re-read with it.
    useEffect(() => {
        if (projectId && topicId && state.phase === 'idle') void refreshGit(agentId, projectId, topicId)
    }, [projectId, topicId, state.phase, refreshGit])

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
        answerQuestions,
        answerSensitive,
        dispatch,
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
    const title = conversationTitle(
        state.title || topics[topicIndex]?.title || projectSessionTitle,
        topicId
    )

    // Deleting the conversation on screen: the server refuses while a turn runs in it.
    const [deleting, setDeleting] = useState<DeleteTarget | null>(null)
    const deleteConversation = topicId ? (
        <IconButton
            icon="trash"
            aria-label="Delete conversation"
            onClick={() => setDeleting({ topic: topicId, title })}
        />
    ) : null

    // The same fresh topic the picker's "New conversation" opens, one click from the header. In
    // a project it is a new project session, created server-side so it is listed immediately.
    const newConversation = (
        <IconButton
            icon="plus"
            aria-label={projectId ? 'New project session' : 'New conversation'}
            onClick={() => {
                if (!projectId) {
                    navigate(`/${agentId}/chat/${nanoid(10)}`)
                    return
                }
                void createProjectSession(agentId, projectId)
                    .then((session) => {
                        insertSession(agentId, projectId, {
                            topic: session.topic,
                            title: session.title,
                            is_thinking: false,
                            last_activity: null,
                        })
                        navigate(`/${agentId}/projects/${projectId}/chat/${session.topic}`)
                    })
                    .catch((e) =>
                        toast.error(
                            "Couldn't start a session",
                            e instanceof Error ? e.message : undefined
                        )
                    )
            }}
        />
    )

    // The recalled memory, background jobs and session files: a side panel, or a sheet on a phone.
    const details = (
        <SideCard>
            {projectId && topicId && gitAvailable && (
                <>
                    <ChangesPanel
                        agentId={agentId}
                        projectId={projectId}
                        topicId={topicId}
                        refreshKey={`${state.phase}:${state.entries.length}`}
                        branchAction={
                            mobile ? (
                                <BranchChip agentId={agentId} projectId={projectId} topicId={topicId} />
                            ) : undefined
                        }
                    />
                    <FadeRule />
                </>
            )}
            <RecalledMemoryPanel agentId={agentId} recalled={recalled} />
            <FadeRule />
            <BackgroundJobsPanel
                jobs={jobs}
                onCancel={(id) =>
                    void cancelJob(agentId, topicId ?? '', id).catch((e) =>
                        toast.error("Couldn't cancel the job", e.message)
                    )
                }
                onJumpToStart={() => {}}
            />
            <FadeRule />
            <SessionFilesPanel
                files={state.files}
                agentId={agentId}
                topicId={topicId ?? ''}
                canPromote={agent ? canManageArtifacts(agent) : false}
                onClear={() => dispatch(agentId, topicId ?? '', { type: 'files', files: [] })}
            />
        </SideCard>
    )

    // A direct project URL without the permission shows the refused state, never empty
    // read-only content (FR-018). Kept after every hook so the hook order never changes.
    if (projectId && agent && !canUseProjects(agent)) return <ProjectsRefused />

    return (
        <div data-chat-area className="flex min-h-0 flex-1 flex-col">
            <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                <ConversationHeader
                    agentId={agentId}
                    title={title}
                    meta={
                        project
                            ? `${project.name} · ${project.folder}`
                            : topicIndex >= 0
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
                            {deleteConversation}
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
                            {projectId && topicId && (
                                <BranchChip
                                    agentId={agentId}
                                    projectId={projectId}
                                    topicId={topicId}
                                />
                            )}
                            <JobsPill
                                count={runningJobs}
                                onClick={() => togglePanel(true)}
                                open={panel}
                            />
                            {deleteConversation}
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

                <HostExecutionNotice className="mx-24 mt-8" />
                {project && !project.available && (
                    <Notice
                        tone="failed"
                        title="The project folder is unavailable"
                        className="mx-24 mt-8"
                    >
                        History stays readable; commands are refused until the folder is restored.
                    </Notice>
                )}

                {/* The conversation runs the full width and the details card floats over its right
                    edge, so the messages' scroll bar sits at the panel's edge, not beside the card. */}
                <div className="relative flex min-h-0 flex-1">
                <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
                <MessageList
                    insetRight={!mobile && panel ? SIDE_PANEL_WIDTH : 0}
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
                    buildingPlan={buildingPlan}
                    onBuildPlan={() => {
                        if (!projectId || !topicId) return
                        setBuildingPlan(true)
                        void setProjectMode(agentId, projectId, topicId, 'build')
                            .then(() => send(agentId, topicId, 'Build the plan.'))
                            .catch((error) =>
                                toast.error(
                                    "Couldn't switch to Build",
                                    error instanceof Error ? error.message : undefined
                                )
                            )
                            .finally(() => setBuildingPlan(false))
                    }}
                    clientCalls={(turn: Turn, running) =>
                        renderClientCalls(turn, running, clientContext)
                    }
                    questions={(turn: Turn) =>
                        renderQuestions(turn, state.refusals, (answer) =>
                            answerQuestions(agentId, topicId ?? '', answer)
                        )
                    }
                    emptyAction={
                        !project && agentProjects.length === 0 ? (
                            <OpenProjectButton agentId={agentId} />
                        ) : undefined
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
                <div
                    ref={composerBox}
                    className="pointer-events-none absolute inset-x-0 bottom-0 z-10 transition-[padding] duration-[var(--vz-duration-slow)]"
                    style={{ paddingRight: !mobile && panel ? SIDE_PANEL_WIDTH : 0 }}
                >
                    <div aria-hidden className="absolute inset-0 bg-gradient-to-t from-panel via-panel/80 to-transparent" />
                    <div className="pointer-events-auto relative mx-auto w-full max-w-4xl px-24 pt-24 pb-16">
                    <SideChatPanel
                        key={sideChat.id}
                        open={sideChat.open}
                        onClose={closeSideChat}
                        agentId={agentId}
                        agentName={agentName}
                        modelName={modelName}
                        channelId={channel}
                        topicId={topicId ?? ''}
                        question={sideChat.question}
                    />
                    <HostExecutionNotice className="mb-8" />
                    <Composer
                        agentId={agentId}
                        agentName={agentName}
                        project={
                            projectId
                                ? {
                                      id: projectId,
                                      mode: projectMode ?? 'plan',
                                      shell: project?.shell_enabled,
                                  }
                                : undefined
                        }
                        topicId={topicId}
                        onModeChange={(mode) => {
                            if (!projectId || !topicId) return
                            void setProjectMode(agentId, projectId, topicId, mode).catch(
                                (error) =>
                                    toast.error(
                                        "Couldn't change the mode",
                                        error instanceof Error ? error.message : undefined
                                    )
                            )
                        }}
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
                        onSend={(text, attachments, spoken, mentions) =>
                            send(
                                agentId,
                                topicId ?? '',
                                text,
                                attachments,
                                spoken,
                                mentions
                            )
                        }
                        onSendAudio={(attachment) =>
                            sendAudio(agentId, topicId ?? '', attachment)
                        }
                        onCommand={(c) => command(agentId, topicId ?? '', c)}
                        onStop={() => command(agentId, topicId ?? '', 'abort')}
                        onSideChat={(question) =>
                            setSideChat({ open: true, id: nanoid(), question })
                        }
                    />
                    </div>
                </div>
                </div>
                <Presence>
                    {!mobile && panel && (
                        <motion.aside
                            key="details"
                            aria-label="Conversation details"
                            className="pointer-events-none absolute inset-y-0 right-0 flex w-400 flex-col gap-24 overflow-x-hidden overflow-y-auto p-16 [&>*]:pointer-events-auto [&>*]:min-w-0"
                            // The conversation makes room for the panel instead of
                            // jumping when it appears.
                            initial={{ width: reducedMotion ? SIDE_PANEL_WIDTH : 0, opacity: reducedMotion ? 1 : 0 }}
                            animate={{ width: SIDE_PANEL_WIDTH, opacity: 1 }}
                            exit={reducedMotion ? { opacity: 0 } : { width: 0, opacity: 0 }}
                            transition={{ duration: motionDurations.slow, ease: motionEases.standard }}
                        >
                            {details}
                        </motion.aside>
                    )}
                </Presence>
                </div>
            </div>

            {chatWorktree && projectId && topicId ? (
                <DeleteWorktreeDialog
                    agentId={agentId}
                    projectId={projectId}
                    worktree={
                        deleting
                            ? { id: chatWorktree.id, name: chatWorktree.name, topic: topicId }
                            : null
                    }
                    onClose={() => setDeleting(null)}
                    onDeleted={() => {
                        setDeleting(null)
                        void loadProjectSessions(agentId, projectId)
                        void refreshProjects(agentId)
                        navigate(`/${agentId}/projects/${projectId}`)
                    }}
                />
            ) : (
                <DeleteConversation
                    agentId={agentId}
                    channel={channel}
                    target={deleting}
                    onClose={() => setDeleting(null)}
                    onDeleted={() =>
                        navigate(
                            projectId
                                ? `/${agentId}/projects/${projectId}`
                                : `/${agentId}/chat/${nanoid(10)}`
                        )
                    }
                />
            )}
            <SensitiveAskDialog
                ask={Object.values(state.asks)[0] ?? null}
                onAnswer={(askId, value) => answerSensitive(agentId, topicId ?? '', askId, value)}
            />
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
