import {
    AgentHeader,
    AgentReply,
    BackgroundReportCard,
    ChatGreeting,
    CheckpointDivider,
    CommandNote,
    HandoverSummary,
    JumpToLatest,
    LoadMore,
    StoppedNote,
    UserMessage,
} from '@vizier/design'
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { motion } from 'motion/react'
import { Presence } from '@vizier/design'
import type { ChatMessage, Turn } from '../../api/frames'
import type { TopicState } from '../../lib/frames'
import {
    isAudioReply,
    isAudioRequest,
    isBackgroundReport,
    replyAttachments,
    replyError,
    replyStats,
    replyText,
    requestAttachments,
    requestText,
    statsLine,
    time,
} from '../../lib/messages'
import { Enter, stagger } from '../../lib/motion'
import { usePrices } from '../../lib/prices'
import { groupHistory, outcomeDurationMs } from '../../lib/trail'
import { personName } from '../../lib/tasks'
import { Markdown } from '../shared/Markdown'
import { ReplyMedia } from './ReplyMedia'
import { Reactions } from './Reactions'
import { Trail } from './Trail'
import { mentionsAsCode } from '../../lib/mentions'
import { planActionsVisible, planOf } from '../../lib/plans'
import { PlanCard } from './client/PlanCard'

type Props = {
    state: TopicState
    /** `live`: the agent is up (live or running), which lights its sigil's dot. */
    agent: { id: string; name: string; live?: boolean }
    /** What an empty topic opens with. */
    greeting?: string
    /** Older history can still be loaded above. */
    hasOlder: boolean
    /** Height of the composer floating over the bottom of the list, which the content scrolls clear of. */
    bottomInset?: number
    /** Room kept free at the right edge for a panel floating over the list. */
    insetRight?: number
    loadingOlder: boolean
    onLoadOlder: () => void
    onReact: (uid: string, emoji: string, action: 'added' | 'removed') => void
    /** The notice under the last reply when the turn ended in an error. */
    error?: ReactNode
    /** Client tool calls a turn made, drawn inside its reply (T056). */
    clientCalls?: (turn: Turn, running: boolean) => ReactNode
    /** The question sets the turn asked, drawn after its reply text (spec 042). */
    questions?: (turn: Turn) => ReactNode
    /** A plan card's Build this plan action (spec 041 US2). */
    onBuildPlan?: (plan: { title: string; steps: string[] }) => void
    /** Build was pressed and the chat is switching mode. */
    buildingPlan?: boolean
    /** How many passages the turn recalled, for its trail summary. */
    recalledFor?: (turn: Turn) => number
    /** A Discord or Telegram conversation: people's messages carry their own names, not "You". */
    byPlatformName?: boolean
    /** Shown under the greeting of an empty conversation, e.g. "Open a Project" on desktop. */
    emptyAction?: ReactNode
}

function stopTime(entry: ChatMessage | undefined) {
    return entry?.timestamp ? time(entry.timestamp) : ''
}

/** The commands that draw their own line; `abort` is named by its StoppedNote instead. */
function isCommand(command: string): command is 'checkpoint' | 'lobotomy' {
    return command === 'checkpoint' || command === 'lobotomy'
}

/**
 * Whether this checkpoint is the one `/lobotomy` saves. The engine stores the command and then
 * a checkpoint with no handover (`run_lobotomy`), so the command directly above it is what tells
 * the two resets apart — a `/checkpoint` whose handover generation failed looks the same
 * otherwise.
 */
function lobotomyAt(entries: ChatMessage[], checkpoint: ChatMessage) {
    const at = entries.findIndex((entry) => entry.uid === checkpoint.uid)
    if (at <= 0) return false
    // Nothing but the command's own entry separates them: both are saved in one go.
    for (let i = at - 1; i >= 0; i -= 1) {
        const command = entries[i].content.Command
        if (command !== undefined) return command === 'lobotomy'
        if (entries[i].content.Checkpoint !== undefined) break
    }
    return false
}

export function MessageList({ state, agent, greeting, hasOlder, bottomInset = 0, insetRight = 0, loadingOlder, onLoadOlder, onReact, error, clientCalls, questions, recalledFor, byPlatformName, emptyAction, onBuildPlan, buildingPlan }: Props) {
    const { entries } = state
    const turns = useMemo(() => groupHistory(entries), [entries])
    const prices = usePrices()
    const scroller = useRef<HTMLDivElement>(null)
    const [pinned, setPinned] = useState(true)
    const [unseen, setUnseen] = useState(0)
    const lastCount = useRef(entries.length)

    // Stay at the bottom while it grows, unless the person scrolled up.
    useEffect(() => {
        const el = scroller.current
        if (!el) return
        if (pinned) {
            el.scrollTop = el.scrollHeight
            setUnseen(0)
        } else if (entries.length > lastCount.current) {
            setUnseen((n) => n + (entries.length - lastCount.current))
        }
        lastCount.current = entries.length
    }, [entries, state.phase, state.queued.length, pinned])

    const byAnchor = new Map<string, Turn>()
    let open: Turn | undefined
    for (const t of turns) {
        if (t.anchorUid) byAnchor.set(t.anchorUid, t)
        else if (!t.outcome && !t.aborted) open = t
    }
    const running = state.phase !== 'idle'

    const reply = (turn: Turn) => {
        const outcome = turn.outcome
        const content = outcome?.content.Response?.content
        const text = content ? replyText(content) : null
        const attachments = replyAttachments(content, outcome?.content.Response?.attachments)
        const spoken = isAudioReply(content)
        const stats = content ? replyStats(content) : undefined
        if (stats?.provider && stats.model) prices.load(stats.provider, stats.model)
        const price = stats?.provider && stats.model ? prices.prices[`${stats.provider}/${stats.model}`] : undefined
        return (
            <div key={`reply-${turn.key}`} className="flex flex-col gap-8">
                {(() => {
                    const plan = planOf(turn)
                    if (!plan) return null
                    const newest = turns[turns.length - 1]?.key ?? null
                    return (
                        <PlanCard
                            plan={plan}
                            actionable={planActionsVisible(turn.key, newest)}
                            building={buildingPlan}
                            onBuild={() => onBuildPlan?.(plan)}
                        />
                    )
                })()}
                {(text != null || attachments.length > 0) && (
                    <AgentReply
                        stats={stats ? statsLine(stats, price) : undefined}
                        onCopy={text != null ? () => void navigator.clipboard?.writeText(text) : undefined}
                        reactions={
                            outcome && (
                                <Reactions
                                    reactions={outcome.reactions ?? []}
                                    disabled={outcome.serverUid === false}
                                    onToggle={(emoji, action) => onReact(outcome.uid, emoji, action)}
                                />
                            )
                        }
                    >
                        <div className="flex flex-col gap-16">
                            {/* A spoken reply's transcript sits behind "Show transcript" in its player. */}
                            {text != null && text !== '' && !spoken && <Markdown>{text}</Markdown>}
                            <ReplyMedia attachments={attachments} transcript={text} />
                        </div>
                    </AgentReply>
                )}
                {outcome?.content.Response?.resumed && <StoppedNote>Picked up where it left off after vizier restarted.</StoppedNote>}
            </div>
        )
    }

    // One turn: the agent's head, then its working folded into the same flow, then what it said.
    const block = (turn: Turn, key: string) => (
        <div key={key} className="flex flex-col gap-10">
            <AgentHeader agentId={agent.id} name={agent.name} live={agent.live} time={time(turn.outcome?.timestamp)} />
            <Trail
                trail={turn.trail}
                live={false}
                durationMs={outcomeDurationMs(turn.outcome)}
                recalled={recalledFor?.(turn)}
                failed={!!(turn.outcome && replyError(turn.outcome.content.Response!.content))}
                aborted={turn.aborted}
                agentName={agent.name}
            />
            {clientCalls?.(turn, false)}
            {turn.outcome && reply(turn)}
            {questions?.(turn)}
        </div>
    )

    const items: ReactNode[] = []
    // Arrivals wait a beat on one another, so a batch of history reads as one
    // movement instead of a pile-up.
    let order = 0
    const arrive = (node: ReactNode, key: string) => {
        items.push(
            <Enter key={key} delay={stagger(order++)}>
                {node}
            </Enter>
        )
    }
    for (const e of entries) {
        const c = e.content
        if (c.Request) {
            const content = c.Request.content
            if ('unattended' in content) continue
            if (isBackgroundReport(content)) {
                const r = content.background_report
                arrive(
                    <BackgroundReportCard
                        key={e.uid}
                        agentId={agent.id}
                        meta={`${r.kind === 'delegation' ? `delegate_agent → ${r.delegated_to ?? ''}` : 'background_subagents'} · ${r.entries.length} piece${r.entries.length === 1 ? '' : 's'}`}
                        linkLabel="What each piece found"
                        details={r.entries.map((en) => (
                            <div key={en.ordinal} className="flex flex-col gap-4">
                                <p className="type-small text-faint">
                                    {en.ordinal + 1}. {en.prompt} · {en.state}
                                </p>
                                <Markdown size="document">{en.text || '_No answer._'}</Markdown>
                                {en.truncated && <p className="type-small text-faint">The answer was cut short here.</p>}
                            </div>
                        ))}
                    />,
                    e.uid
                )
                continue
            }
            const skill = (c.Request.metadata as { invoked_skill?: string } | undefined)?.invoked_skill
            const mentioned = (c.Request.metadata as { mentions?: string[] } | undefined)?.mentions
            const attachments = requestAttachments(content, c.Request.attachments)
            const spoken = isAudioRequest(content)
            const text = requestText(content)
            arrive(
                <UserMessage
                    key={e.uid}
                    meta={`${byPlatformName && c.Request.user ? personName(c.Request.user) : 'You'} · ${time(c.Request.timestamp)}`}
                    skill={skill}
                    attachments={attachments.length ? <ReplyMedia attachments={attachments} transcript={spoken ? text : undefined} /> : undefined}
                >
                    {/* A voice message's transcript sits behind "Show transcript" in its player. */}
                    {!spoken && text !== '' && <Markdown size="document">{mentionsAsCode(text, mentioned)}</Markdown>}
                </UserMessage>,
                e.uid
            )
            continue
        }
        const turn = byAnchor.get(e.uid)
        if (turn && (c.Response || c.Checkpoint !== undefined || c.Command !== undefined)) {
            // The finished turn settles in as one piece: the work it did and
            // what it said arrive together.
            if (c.Response || turn.trail.length || turn.clientCalls.length || turn.questions.length) arrive(block(turn, `turn-${e.uid}`), `turn-${e.uid}`)
            if (c.Command === 'abort' && turn.aborted) arrive(<StoppedNote key={`stop-${e.uid}`}>Stopped by you at {stopTime(e)}</StoppedNote>, `stop-${e.uid}`)
        }
        if (c.Checkpoint !== undefined) {
            const handover = typeof c.Checkpoint === 'string' ? c.Checkpoint : c.Checkpoint?.handover
            // `/lobotomy` asks for exactly this: a checkpoint that carries nothing over. The
            // divider says which of the two it was, because they read the same without it.
            const kind = lobotomyAt(entries, e) ? 'lobotomy' : 'checkpoint'
            arrive(<CheckpointEntry key={`cp-${e.uid}`} handover={handover ?? null} kind={kind} />, `cp-${e.uid}`)
        }
        // A command the person ran: `/checkpoint` and `/lobotomy` are written out, since the
        // divider alone does not say one was asked for. An abort already has its StoppedNote.
        if (c.Command && c.Command !== 'abort' && isCommand(c.Command)) {
            arrive(<CommandNote key={`cmd-${e.uid}`} command={c.Command} meta={time(e.timestamp)} />, `cmd-${e.uid}`)
        }
    }

    if (open && (running || open.trail.length || open.clientCalls.length || open.questions.length)) {
        items.push(
            <div key="live" className="flex flex-col gap-10">
                <AgentHeader agentId={agent.id} name={agent.name} live={agent.live} />
                <Trail trail={open.trail} live recalled={recalledFor?.(open)} agentName={agent.name} />
                {clientCalls?.(open, true)}
                {questions?.(open)}
            </div>
        )
    } else if (running) {
        items.push(
            <div key="live-empty" className="flex flex-col gap-10">
                <AgentHeader agentId={agent.id} name={agent.name} live={agent.live} />
                <Trail trail={[]} live agentName={agent.name} />
            </div>
        )
    }

    state.queued.forEach((q) =>
        arrive(
            <UserMessage
                key={`q-${q.id}`}
                queued={{ agentName: agent.name }}
                attachments={q.attachments?.length ? <ReplyMedia attachments={q.attachments} /> : undefined}
            >
                <Markdown size="document">{q.text}</Markdown>
            </UserMessage>,
            `q-${q.id}`
        )
    )
    if (error) arrive(<div key="error">{error}</div>, 'error')

    return (
        <div className="relative flex min-h-0 flex-1 flex-col">
            <div
                ref={scroller}
                className="flex min-h-0 flex-1 flex-col overflow-y-auto px-24 pt-24"
                style={{ paddingRight: 24 + insetRight }}
                onScroll={(e) => {
                    const el = e.currentTarget
                    setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < 48)
                }}
            >
                <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-20">
                    {hasOlder && <LoadMore look="auto" label="Load older" loading={loadingOlder} onLoad={onLoadOlder} />}
                    {items.length === 0 && !running ? (
                        <div className="my-auto flex flex-col items-center gap-20">
                            <ChatGreeting agentId={agent.id} name={agent.name} live={agent.live} greeting={greeting} meta="Nothing sent yet" />
                            {emptyAction}
                        </div>
                    ) : (
                        items
                    )}
                </div>
                {/* Room for the composer floating over the list. A spacer, not padding-bottom: WebKit
                    leaves a flex scroll container's end padding out of what can be scrolled to. */}
                <div aria-hidden className="shrink-0 transition-[height] duration-[var(--vz-duration-slow)]" style={{ height: bottomInset + 16 }} />
            </div>
            <Presence>
                {!pinned && (
                    <motion.div
                        key="jump"
                        className="absolute inset-x-0 flex justify-center"
                        style={{ bottom: bottomInset + 8 }}
                        initial={{ opacity: 0, y: 8 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: 8 }}
                    >
                        <JumpToLatest
                            count={unseen}
                            onClick={() => {
                                const el = scroller.current
                                if (el) el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' })
                                setPinned(true)
                            }}
                        />
                    </motion.div>
                )}
            </Presence>
        </div>
    )
}

function CheckpointEntry({ handover, kind }: { handover: string | null; kind: 'checkpoint' | 'lobotomy' }) {
    const [open, setOpen] = useState(false)
    return (
        <div className="flex flex-col gap-8">
            <CheckpointDivider kind={kind} detail={handover ? 'handed over' : undefined} open={open} onClick={() => handover && setOpen((o) => !o)} />
            <Presence>
                {open && handover && (
                    <motion.div
                        key="handover"
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        className="overflow-hidden"
                    >
                        <HandoverSummary>
                            <Markdown>{handover}</Markdown>
                        </HandoverSummary>
                    </motion.div>
                )}
            </Presence>
        </div>
    )
}

