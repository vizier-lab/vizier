import {
    AgentHeader,
    AgentReply,
    Attachment,
    BackgroundReportCard,
    ChatGreeting,
    CheckpointDivider,
    HandoverSummary,
    JumpToLatest,
    LoadMore,
    StoppedNote,
    UserMessage,
} from '@vizier/design'
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type { ChatMessage, Turn } from '../../api/frames'
import type { TopicState } from '../../lib/frames'
import {
    isBackgroundReport,
    replyError,
    replyStats,
    replyText,
    requestText,
    statsLine,
    time,
} from '../../lib/messages'
import { usePrices } from '../../lib/prices'
import { groupHistory, outcomeDurationMs } from '../../lib/trail'
import { Markdown } from '../shared/Markdown'
import { ReplyMedia } from './ReplyMedia'
import { Reactions } from './Reactions'
import { Trail } from './Trail'

type Props = {
    state: TopicState
    agent: { id: string; name: string }
    /** Older history can still be loaded above. */
    hasOlder: boolean
    loadingOlder: boolean
    onLoadOlder: () => void
    onReact: (uid: string, emoji: string, action: 'added' | 'removed') => void
    /** The notice under the last reply when the turn ended in an error. */
    error?: ReactNode
    /** Client tool calls a turn made, drawn inside its reply (T056). */
    clientCalls?: (turn: Turn, running: boolean) => ReactNode
    /** How many passages the turn recalled, for its trail summary. */
    recalledFor?: (turn: Turn) => number
    jobHref: (agentId: string, topic: string) => string
}

function stopTime(entry: ChatMessage | undefined) {
    return entry?.timestamp ? time(entry.timestamp) : ''
}

export function MessageList({ state, agent, hasOlder, loadingOlder, onLoadOlder, onReact, error, clientCalls, recalledFor, jobHref }: Props) {
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
        const stats = content ? replyStats(content) : undefined
        if (stats?.provider && stats.model) prices.load(stats.provider, stats.model)
        const price = stats?.provider && stats.model ? prices.prices[`${stats.provider}/${stats.model}`] : undefined
        return (
            <div key={`reply-${turn.key}`} className="flex flex-col gap-8">
                <AgentHeader agentId={agent.id} name={agent.name} time={time(outcome?.timestamp)} />
                {(text != null || (outcome?.content.Response?.attachments?.length ?? 0) > 0) && (
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
                        {text != null && <Markdown>{text}</Markdown>}
                        <ReplyMedia attachments={outcome?.content.Response?.attachments ?? []} transcript={text} />
                        {clientCalls?.(turn, false)}
                    </AgentReply>
                )}
                {outcome?.content.Response?.resumed && <StoppedNote>Picked up where it left off after vizier restarted.</StoppedNote>}
            </div>
        )
    }

    const block = (turn: Turn, key: string) => (
        <div key={key} className="flex flex-col gap-12">
            <Trail
                trail={turn.trail}
                live={false}
                durationMs={outcomeDurationMs(turn.outcome)}
                recalled={recalledFor?.(turn)}
                failed={!!(turn.outcome && replyError(turn.outcome.content.Response!.content))}
                aborted={turn.aborted}
            />
            {turn.outcome ? reply(turn) : clientCalls?.(turn, false)}
        </div>
    )

    const items: ReactNode[] = []
    for (const e of entries) {
        const c = e.content
        if (c.Request) {
            const content = c.Request.content
            if ('unattended' in content) continue
            if (isBackgroundReport(content)) {
                const r = content.background_report
                items.push(
                    <BackgroundReportCard
                        key={e.uid}
                        agentId={agent.id}
                        meta={`${r.kind === 'delegation' ? `delegate_agent → ${r.delegated_to ?? ''}` : 'paralel_subtasks'} · ${r.entries.length} piece${r.entries.length === 1 ? '' : 's'}`}
                        linkLabel="Open the job's conversations"
                        href={jobHref(agent.id, r.job_id)}
                    />
                )
                continue
            }
            const skill = (c.Request.metadata as { invoked_skill?: string } | undefined)?.invoked_skill
            items.push(
                <UserMessage
                    key={e.uid}
                    meta={`You · ${time(c.Request.timestamp)}`}
                    skill={skill}
                    attachments={
                        c.Request.attachments?.length ? (
                            <>
                                {c.Request.attachments.map((a) => (
                                    <Attachment key={a.filename} name={a.filename} />
                                ))}
                            </>
                        ) : undefined
                    }
                >
                    {requestText(content)}
                </UserMessage>
            )
            continue
        }
        const turn = byAnchor.get(e.uid)
        if (turn && (c.Response || c.Checkpoint !== undefined || c.Command !== undefined)) {
            if (c.Response || turn.trail.length || turn.clientCalls.length) items.push(block(turn, `turn-${e.uid}`))
            if (c.Command === 'abort' && turn.aborted) items.push(<StoppedNote key={`stop-${e.uid}`}>Stopped by you at {stopTime(e)}</StoppedNote>)
        }
        if (c.Checkpoint !== undefined) {
            const handover = typeof c.Checkpoint === 'string' ? c.Checkpoint : c.Checkpoint?.handover
            items.push(<CheckpointEntry key={`cp-${e.uid}`} handover={handover ?? null} />)
        }
    }

    if (open && (running || open.trail.length || open.clientCalls.length)) {
        items.push(
            <div key="live" className="flex flex-col gap-12">
                <Trail trail={open.trail} live recalled={recalledFor?.(open)} />
                {clientCalls?.(open, true)}
            </div>
        )
    } else if (running) {
        items.push(<Trail key="live-empty" trail={[]} live />)
    }

    state.queued.forEach((q) =>
        items.push(
            <UserMessage key={`q-${q.id}`} queued={{ agentName: agent.name }}>
                {q.text}
            </UserMessage>
        )
    )
    if (error) items.push(<div key="error">{error}</div>)

    return (
        <div className="relative flex min-h-0 flex-1 flex-col">
            <div
                ref={scroller}
                className="flex min-h-0 flex-1 flex-col gap-20 overflow-y-auto px-24 py-24"
                onScroll={(e) => {
                    const el = e.currentTarget
                    setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < 48)
                }}
            >
                {hasOlder && <LoadMore look="auto" label="Load older" loading={loadingOlder} onLoad={onLoadOlder} />}
                {items.length === 0 && !running ? <ChatGreeting agentId={agent.id} name={agent.name} meta="Nothing sent yet" /> : items}
            </div>
            {!pinned && (
                <div className="absolute inset-x-0 bottom-12 flex justify-center">
                    <JumpToLatest
                        count={unseen}
                        onClick={() => {
                            const el = scroller.current
                            if (el) el.scrollTop = el.scrollHeight
                            setPinned(true)
                        }}
                    />
                </div>
            )}
        </div>
    )
}

function CheckpointEntry({ handover }: { handover: string | null }) {
    const [open, setOpen] = useState(false)
    return (
        <div className="flex flex-col gap-8">
            <CheckpointDivider detail={handover ? 'handed over' : undefined} open={open} onClick={() => handover && setOpen((o) => !o)} />
            {open && handover && (
                <HandoverSummary>
                    <Markdown>{handover}</Markdown>
                </HandoverSummary>
            )}
        </div>
    )
}

