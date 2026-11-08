import { DeleteConversation, type DeleteTarget } from '../screens/chat/DeleteConversation'
import { DeleteWorktreeDialog } from '../screens/projects/DeleteWorktreeDialog'
import { IconButton, Notice } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { agentState, getAgent } from '../api/endpoints/agents'
import { CHANNEL, listTopics, restartAgent } from '../api/endpoints/chat'
import { listIntegrations } from '../api/endpoints/integrations'
import { createProjectSession, projectChannel } from '../api/endpoints/projects'
import type { Topic, Turn } from '../api/frames'
import type { ClientCallView } from '../api/frames'
import { emptyTopic } from '../lib/frames'
import { greeting } from '../lib/greetings'
import { requestText } from '../lib/messages'
import { ConversationHeader } from '../screens/chat/ConversationHeader'
import { BranchChip } from '../screens/chat/BranchChip'
import { ChangesSection } from '../screens/chat/ChangesSection'
import { AgentStateBanner } from '../screens/chat/AgentStateBanner'
import { renderClientCalls } from '../screens/chat/ClientCalls'
import { renderQuestions } from '../screens/chat/Questions'
import { SensitiveAskDialog } from '../components/questions/SensitiveAskDialog'
import { Composer } from '../screens/chat/Composer'
import { SideChatPanel } from '../screens/chat/SideChatPanel'
import { ErrorNotice } from '../screens/chat/ErrorNotice'
import { MessageList } from '../screens/chat/MessageList'
import { recalledFrom } from '../lib/recall'
import { ChatStatusStrip } from '../screens/chat/ChatStatusStrip'
import { ModelOverridePicker } from '../screens/chat/ModelOverridePicker'
import { readModelOverride, writeModelOverride } from '../lib/modelOverride'
import type { ModelOverride } from '../api/frames'
import { HostExecutionNotice } from '../screens/projects/HostExecutionNotice'
import {
    OpenProjectButton,
    ProjectsRefused,
    shouldOfferOpenProject,
} from '../screens/projects/ProjectsSection'
import { lastTopic, rememberTopic } from '../lib/remember'
import { useAgents } from '../stores/agents'
import { topicKey, useConnection } from '../stores/connection'
import { useSession } from '../stores/session'
import { projectKey, useProjects } from '../stores/projects'
import { toast } from '../stores/toasts'
import { canManageArtifacts, canUseProjects } from '../lib/access'
import { useTurns } from '../stores/turns'
import { conversationTitle } from '../lib/conversations'
import { useMobile } from '../lib/useMobile'

const EMPTY = emptyTopic()
// The strip's own margin under the composer, and the air between it and the panel.
const STRIP_MARGIN = 8
const PANEL_GAP = 8

export default function Chat() {
    const { agentId = '', topicId, projectId } = useParams()
    const navigate = useNavigate()
    // A project session is the same screen on the project's own channel (spec 040).
    const channel = projectId ? projectChannel(projectId) : CHANNEL
    const project = useProjects((s) =>
        projectId ? (s.byAgent[agentId] ?? []).find((p) => p.id === projectId) : undefined
    )
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
    const gitSummary = useProjects((s) =>
        projectId && topicId ? s.git[`${projectKey(agentId, projectId)}/${topicId}`] : undefined
    )
    const gitAvailable = !!gitSummary?.available
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

    // The host-execution notice is only worth its line when this agent can actually run a
    // command: the Shell addon, or a project whose own shell switch is on. Nothing is drawn
    // until the addon list has arrived; if that read fails, the notice stays, because hiding a
    // disclosure is the worse mistake.
    const [shellAddon, setShellAddon] = useState<boolean>()
    useEffect(() => {
        let alive = true
        setShellAddon(undefined)
        listIntegrations(agentId, 'addon')
            .then((list) => {
                if (alive)
                    setShellAddon((list?.items ?? []).some((i) => i.kind === 'shell' && i.enabled))
            })
            .catch(() => {
                if (alive) setShellAddon(true)
            })
        return () => {
            alive = false
        }
    }, [agentId])

    const [topics, setTopics] = useState<Topic[]>([])
    const userId = useSession((s) => s.user?.user_id)
    const canOverrideModel = useSession((s) => s.user?.permissions.includes('model_override') ?? false)
    const preferenceKey = useMemo(() => userId && topicId ? {
        userId,
        agentId,
        channelId: channel,
        topicId,
    } : null, [userId, agentId, channel, topicId])
    const [modelOverride, setModelOverride] = useState<ModelOverride | null>(null)
    const [modelChoiceAvailable, setModelChoiceAvailable] = useState(true)
    useEffect(() => {
        setModelOverride(preferenceKey ? readModelOverride(preferenceKey) : null)
    }, [preferenceKey])
    const changeModelOverride = useCallback((choice: ModelOverride | null) => {
        setModelOverride(choice)
        if (preferenceKey) writeModelOverride(preferenceKey, choice)
    }, [preferenceKey])
    // A project's changes open in their own dialog, behind the status strip's count.
    const [changesOpen, setChangesOpen] = useState(false)
    // Stable, so the panel's Escape listener is not re-bound on every render.
    const closeSideChat = useCallback(() => setSideChat((s) => (s.open ? { ...s, open: false } : s)), [])
    const composerBox = useRef<HTMLDivElement>(null)
    const [composerHeight, setComposerHeight] = useState(120)
    useEffect(() => {
        const el = composerBox.current
        if (!el) return
        const ro = new ResizeObserver(() => setComposerHeight(el.offsetHeight))
        ro.observe(el)
        return () => ro.disconnect()
    }, [])
    // The status panel hangs above the composer, so it must clear the composer's height.
    const composerCard = useRef<HTMLDivElement>(null)
    const [composerCardHeight, setComposerCardHeight] = useState(84)
    // The strip shares its row with the model picker, so it is only as wide as its counts:
    // the panel takes the composer's width instead of the strip's.
    const [composerCardWidth, setComposerCardWidth] = useState<number | undefined>(undefined)
    useEffect(() => {
        const el = composerCard.current
        if (!el) return
        const measure = () => {
            setComposerCardHeight(el.offsetHeight)
            setComposerCardWidth(el.offsetWidth)
        }
        measure()
        const ro = new ResizeObserver(measure)
        ro.observe(el)
        return () => ro.disconnect()
    }, [])
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
    // On a phone the two sit bare at the end of the topic row (spec 056: no control has a border).
    const mobile = useMobile()
    const headerButton = mobile ? ('ghost' as const) : undefined
    const deleteConversation = topicId ? (
        <IconButton
            variant={headerButton}
            icon="trash"
            aria-label="Delete conversation"
            onClick={() => setDeleting({ topic: topicId, title })}
        />
    ) : null

    // The same fresh topic the picker's "New conversation" opens, one click from the header. In
    // a project it is a new project session, created server-side so it is listed immediately.
    const newConversation = (
        <IconButton
            variant={headerButton}
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

    // What the agent recalled, what it is still working on, what it left behind and
    // what it changed all live in the status strip under the composer.
    const changes =
        projectId && topicId && gitAvailable
            ? {
                  count: gitSummary?.changed ?? 0,
                  open: () => setChangesOpen(true),
              }
            : undefined

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
                    share={
                        topicId
                            ? { topicId, agent, agentName, empty: state.entries.length === 0 && state.phase === 'idle' }
                            : undefined
                    }
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
                            {deleteConversation}
                            {newConversation}
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
                            {deleteConversation}
                            {newConversation}
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
                        shouldOfferOpenProject(projectId) ? (
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
                    className="pointer-events-none absolute inset-x-0 bottom-0 z-10"
                >
                    <div aria-hidden className="absolute inset-0 bg-gradient-to-t from-panel via-panel/80 to-transparent" />
                    <div className="pointer-events-auto relative mx-auto w-full max-w-4xl px-24 pt-24 pb-16 max-lg:px-12 max-lg:pt-16 max-lg:pb-4">
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
                    <HostExecutionNotice
                        shell={!!shellAddon || !!project?.shell_enabled}
                        className="mb-8"
                    />
                    <div ref={composerCard}>
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
                                  : canOverrideModel && modelOverride && channel === CHANNEL && !modelChoiceAvailable
                                    ? 'The selected model is unavailable. Choose another model or Agent default.'
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
                                mentions,
                                canOverrideModel && channel && userId ? modelOverride : null
                            )
                        }
                        onSendAudio={(attachment) =>
                            sendAudio(
                                agentId,
                                topicId ?? '',
                                attachment,
                                canOverrideModel && channel && userId ? modelOverride : null
                            )
                        }
                        onCommand={(c) => command(agentId, topicId ?? '', c)}
                        onStop={() => command(agentId, topicId ?? '', 'abort')}
                        onSideChat={(question) =>
                            setSideChat({ open: true, id: nanoid(), question })
                        }
                    />
                    </div>
                    {/* The status strip: what the agent recalled, what it is still working
                        on, what it left in this conversation and what it changed. */}
                    <div className="mt-8 flex min-w-0 items-center gap-16">
                    <ChatStatusStrip
                        className="shrink-0"
                        agentId={agentId}
                        topicId={topicId ?? ''}
                        recalled={recalled}
                        jobs={jobs}
                        files={state.files}
                        canPromote={agent ? canManageArtifacts(agent) : false}
                        onFilesCleared={() => dispatch(agentId, topicId ?? '', { type: 'files', files: [] })}
                        changes={changes}
                        lsp={projectId && project ? { projectId, enabled: project.lsp_enabled } : undefined}
                        // The panel clears the composer and the strip's own margin.
                        panelGap={composerCardHeight + STRIP_MARGIN + PANEL_GAP}
                        panelWidth={composerCardWidth}
                    />
                    {canOverrideModel && userId && topicId && (
                        <>
                            <ModelOverridePicker
                                defaultModel={health?.model ?? null}
                                value={modelOverride}
                                onChange={changeModelOverride}
                                onAvailabilityChange={setModelChoiceAvailable}
                            />
                        </>
                    )}
                    </div>
                    </div>
                </div>
                </div>
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
            {projectId && topicId && (
                <ChangesSection
                    agentId={agentId}
                    projectId={projectId}
                    topicId={topicId}
                    open={changesOpen}
                    onClose={() => setChangesOpen(false)}
                    refreshKey={`${state.phase}:${state.entries.length}`}
                />
            )}
        </div>
    )
}
