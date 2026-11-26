import {
    AgentHeader,
    AgentReply,
    BackgroundReportCard,
    ChatGreeting,
    CheckpointDivider,
    HandoverSummary,
    JumpToLatest,
    LoadMore,
    PromptRuler,
    StoppedNote,
    UserMessage,
} from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { motion } from 'motion/react'
import { Presence } from '@vizier/design'
import type { ChatMessage, Turn } from '../../api/frames'
import type { TopicState } from '../../lib/frames'
import {
    isAudioReply,
    isAudioRequest,
    isBackgroundReport,
    isDetachedDelivery,
    replyAttachments,
    replyError,
    replyStats,
    replyText,
    requestAttachments,
    requestText,
    statsLine,
    time,
} from '../../lib/messages'
import { parseDelivery } from '../../lib/detached'
import { splitDelivery } from '../../lib/subagents'
import { Enter, stagger } from '../../lib/motion'
import { usePrices } from '../../lib/prices'
import { blockDurationMs, groupHistory, lastBlockOf, orderTurnsBySent, outcomeDurationMs } from '../../lib/trail'
import { personName } from '../../lib/tasks'
import { useSession } from '../../stores/session'
import { Markdown } from '../shared/Markdown'
import { PendingAudio, ReplyMedia } from './ReplyMedia'
import { Reactions } from './Reactions'
import { DeliveryRow, Trail, WorkerUpdateRow } from './Trail'
import { DetachedSource } from './detachedSource'
import { mentionsAsCode } from '../../lib/mentions'
import { collectEntityLinks, EntityBroken, rewriteEntityLinks } from '../../lib/entityLinks'
import { useLinkTargets } from '../../stores/linkTargets'
import { planActionsVisible, planOf } from '../../lib/plans'
import { PlanCard } from './client/PlanCard'
import { currentPrompt, markTime, promptIndex, waitingUid, withWaiting, type PromptMark } from '../../lib/prompts'

type Props = {
    /** A live voice call is running here (spec 055): its newest spoken reply draws as a waiting player. */
    liveCall?: boolean
    /** The call is speaking the newest reply now, so the reply's card plays along. */
    liveSpeaking?: boolean
    /** The conversation, so a delivered script's row can read its record (spec 058). */
    topicId?: string
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
    /** Told the width of the list's scroll bar, so what floats over the list can centre on the same column. */
    onScrollbarWidth?: (px: number) => void
    loadingOlder: boolean
    onLoadOlder: () => void
    onReact: (uid: string, emoji: string, action: 'added' | 'removed') => void
    /** The notice under the last reply when the turn ended in an error. */
    error?: ReactNode
    /** Client tool calls a turn made, drawn inside its reply (T056). */
    clientCalls?: (turn: Turn, running: boolean) => ReactNode
    /** The question sets the turn asked, drawn after its reply text (spec 042). */
    questions?: (turn: Turn) => ReactNode
    /** The page versions the turn drew, drawn as its reply (spec 063). */
    pages?: (turn: Turn) => ReactNode
    /** A plan card's Build this plan action (spec 041 US2). */
    onBuildPlan?: (plan: { title: string; steps: string[] }) => void
    /** Build was pressed and the chat is switching mode. */
    buildingPlan?: boolean
    /** A Discord or Telegram conversation: people's messages carry their own names, not "You". */
    byPlatformName?: boolean
    /** Shown under the greeting of an empty conversation, e.g. "Open a Project" on desktop. */
    emptyAction?: ReactNode
    /** Every prompt the person sent, as the server lists them: the prompt ruler's marks. */
    prompts?: PromptMark[]
    /** Load the history back to a prompt that is not on the page yet, to jump to it. */
    onLoadThrough?: (mark: PromptMark) => Promise<void>
}

function stopTime(entry: ChatMessage | undefined) {
    return entry?.timestamp ? time(entry.timestamp) : ''
}

/** The commands that draw their own line; `abort` is named by its StoppedNote instead. */
function isCommand(command: string): command is 'checkpoint' | 'lobotomy' {
    return command === 'checkpoint' || command === 'lobotomy'
}

/** One shared empty set: a context value that is a fresh object re-renders on every call. */
const NO_BROKEN: ReadonlySet<string> = new Set()

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

/** Whether a turn has more than one part, so each trail shows its own time and the reply the whole. */
function inParts(turn: Turn) {
    return turn.blocks.length > 1
}

export function MessageList({ liveCall, liveSpeaking, state, agent, topicId, greeting, hasOlder, bottomInset = 0, insetRight = 0, onScrollbarWidth, loadingOlder, onLoadOlder, onReact, error, clientCalls, questions, pages, byPlatformName, emptyAction, onBuildPlan, buildingPlan, prompts, onLoadThrough }: Props) {
    const { entries } = state
    const me = useSession((s) => s.user)
    const turns = useMemo(() => orderTurnsBySent(groupHistory(entries)), [entries])
    const prices = usePrices()

    // Entity wiki links on the page (spec 066): the store's index answers what it can, one
    // batched resolve settles the rest, and a target that is gone draws broken instead of
    // navigating nowhere. Unresolved renders optimistically live.
    const linkTargetsLoaded = useLinkTargets((s) => s.loaded[agent.id])
    const refreshLinkTargets = useLinkTargets((s) => s.refresh)
    const resolveLinks = useLinkTargets((s) => s.resolve)
    const pageLinks = useMemo(() => {
        const texts: string[] = []
        for (const e of entries) {
            const c = e.content
            if (c.Request) texts.push(requestText(c.Request.content))
            else if (c.Response) {
                const t = replyText(c.Response.content)
                if (t) texts.push(t)
            } else if (typeof c.AssistantMessage === 'string') texts.push(c.AssistantMessage)
        }
        return texts.flatMap(collectEntityLinks)
    }, [entries])
    const [broken, setBroken] = useState<ReadonlySet<string>>(NO_BROKEN)
    useEffect(() => {
        if (!linkTargetsLoaded) {
            void refreshLinkTargets(agent.id)
            return
        }
        if (pageLinks.length === 0) {
            setBroken(NO_BROKEN)
            return
        }
        let alive = true
        void resolveLinks(agent.id, pageLinks).then((map) => {
            if (!alive) return
            const next = new Set(Object.entries(map).filter(([, ok]) => !ok).map(([key]) => key))
            setBroken(next.size > 0 ? next : NO_BROKEN)
        })
        return () => {
            alive = false
        }
    }, [agent.id, linkTargetsLoaded, pageLinks, refreshLinkTargets, resolveLinks])
    const scroller = useRef<HTMLDivElement>(null)
    // The scroller as state too, so the scroll-bar measurement starts when the element attaches,
    // not only on mount (the ref can still be empty then, which left the composer misaligned).
    const [scrollEl, setScrollEl] = useState<HTMLDivElement | null>(null)
    const attachScroller = useCallback((el: HTMLDivElement | null) => {
        scroller.current = el
        setScrollEl(el)
    }, [])
    useEffect(() => {
        const el = scrollEl
        if (!el || !onScrollbarWidth) return
        const measure = () => onScrollbarWidth(el.offsetWidth - el.clientWidth)
        measure()
        // The bar comes and goes with the content's height, not only the box's size.
        const ro = new ResizeObserver(measure)
        ro.observe(el)
        if (el.firstElementChild) ro.observe(el.firstElementChild)
        return () => ro.disconnect()
    }, [scrollEl, onScrollbarWidth])
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
    // The deliveries that started a turn of their own, while the agent was idle.
    const wakers = new Set(turns.flatMap((t) => (t.request ? [t.request.uid] : [])))
    // Messages a running turn read mid-turn (spec 062): drawn inside that turn, between its parts.
    const steered = new Set(turns.flatMap((t) => t.blocks.flatMap((b) => (b.kind === 'steer' ? [b.id] : []))))

    // A person's message, in the conversation or between the parts of the turn that read it.
    const personMessage = (e: ChatMessage) => {
        const request = e.content.Request!
        const content = request.content
        const mentioned = (request.metadata as { mentions?: string[] } | undefined)?.mentions
        const attachments = requestAttachments(content, request.attachments)
        const spoken = isAudioRequest(content)
        const text = requestText(content)
        return (
            <UserMessage
                key={e.uid}
                data-prompt-uid={e.uid}
                className="scroll-mt-24"
                name={byPlatformName && request.user ? personName(request.user) : 'You'}
                seed={request.user || me?.username}
                time={time(request.timestamp)}
                attachments={
                    attachments.length ? (
                        <ReplyMedia
                            attachments={attachments}
                            transcript={spoken ? text : undefined}
                            // A drawn voice message with no text yet is still being recognised.
                            transcribing={spoken && !text && e.uid.startsWith('live-')}
                        />
                    ) : undefined
                }
            >
                {/* A voice message's transcript sits behind "Show transcript" in its player. */}
                {!spoken && text !== '' && (
                    <Markdown size="document" slashWords>
                        {rewriteEntityLinks(agent.id, mentionsAsCode(text, mentioned))}
                    </Markdown>
                )}
            </UserMessage>
        )
    }

    const reply = (turn: Turn) => {
        const outcome = turn.outcome
        const content = outcome?.content.Response?.content
        const text = content ? replyText(content) : null
        const attachments = replyAttachments(content, outcome?.content.Response?.attachments)
        const spoken = isAudioReply(content)
        // A live voice reply whose recording is not kept yet: the same card from the start, waiting
        // for its player, instead of text that turns into a card when the speaking ends.
        const said = turn.request?.content.Request
        const waitingForAudio =
            !!liveCall &&
            !spoken &&
            !!text &&
            turns[turns.length - 1]?.key === turn.key &&
            !!said &&
            (said.spoken === true || 'audio_chat' in said.content)
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
                        stats={stats ? statsLine(stats, price, inParts(turn) ? outcomeDurationMs(outcome) : undefined) : undefined}
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
                            {/* A spoken reply's transcript is open in its player, read along as it is heard. */}
                            {text != null && text !== '' && !spoken && !waitingForAudio && (
                                <Markdown>{rewriteEntityLinks(agent.id, text)}</Markdown>
                            )}
                            {waitingForAudio && text && <PendingAudio transcript={text} speaking={liveSpeaking} />}
                            <ReplyMedia attachments={attachments} transcript={text} openTranscript />
                        </div>
                    </AgentReply>
                )}
                {outcome?.content.Response?.resumed && <StoppedNote>Picked up where it left off after vizier restarted.</StoppedNote>}
            </div>
        )
    }

    // A turn's parts in the order they happened: each stretch of work folded into the flow, and
    // what the agent said on the way at the size of its reply. `live`: the last part is still open.
    const parts = (turn: Turn, live: boolean) => {
        const lastTrail = lastBlockOf(turn, 'trail')
        const failed = !!(turn.outcome && replyError(turn.outcome.content.Response!.content))
        return turn.blocks.map((block, i) => {
            if (block.kind === 'message') return <Markdown key={block.id}>{rewriteEntityLinks(agent.id, block.text)}</Markdown>
            if (block.kind === 'steer') {
                // The agent carries on after the person's message: its head starts the next part,
                // as it starts the turn. Only after the last of messages read together.
                // Spaced like separate messages (20px), not like the parts of one turn (10px); the
                // head's time is when the agent read the message, the row's own time.
                const resumes = turn.blocks[i + 1]?.kind !== 'steer'
                return (
                    <div key={block.id} className="mt-10 flex flex-col gap-10">
                        {personMessage(block.entry)}
                        {resumes && (
                            <div className="mt-10">
                                <AgentHeader agentId={agent.id} name={agent.name} live={agent.live} time={time(block.entry.timestamp)} />
                            </div>
                        )}
                    </div>
                )
            }
            const open = live && i === turn.blocks.length - 1
            return (
                <Trail
                    // A trail that stops being the open one folds, which only a new mount does.
                    key={`${block.events[0]?.id ?? i}-${open ? 'open' : 'done'}`}
                    trail={block.events}
                    live={open}
                    turnRunning={live}
                    durationMs={inParts(turn) ? blockDurationMs(block) : (outcomeDurationMs(turn.outcome) ?? blockDurationMs(block))}
                    failed={i === lastTrail && failed}
                    aborted={i === lastTrail && turn.aborted}
                    agentName={agent.name}
                    agentId={agent.id}
                />
            )
        })
    }

    // One turn: the agent's head, then its parts, then what it said at the end.
    const block = (turn: Turn, key: string) => (
        <div key={key} className="flex flex-col gap-10">
            <AgentHeader agentId={agent.id} name={agent.name} live={agent.live} time={time(turn.outcome?.timestamp)} />
            {parts(turn, false)}
            {clientCalls?.(turn, false)}
            {pages?.(turn)}
            {/* The card is part of what the agent said, so the reply's reactions and model line close the turn below it. */}
            {questions?.(turn)}
            {turn.outcome && reply(turn)}
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
            if (isDetachedDelivery(content)) {
                // Delivered mid-turn, it is a step of that turn's trail (`groupHistory`); only a
                // delivery that woke an idle agent is drawn here, as the card its turn answers.
                if (!wakers.has(e.uid)) continue
                // One delivery may carry finished scripts and worker updates (spec 059).
                const parts = splitDelivery(content.detached_delivery)
                const scripts = parseDelivery(parts.scripts)
                const workers = parts.workers
                const meta = [
                    scripts.length ? (scripts.length === 1 ? '1 script' : `${scripts.length} scripts`) : null,
                    workers.length ? (workers.length === 1 ? '1 subagent' : `${workers.length} subagents`) : null,
                ]
                    .filter(Boolean)
                    .join(' · ')
                arrive(
                    <BackgroundReportCard
                        key={e.uid}
                        agentId={agent.id}
                        title={workers.length && !scripts.length ? 'Subagent update' : 'Background work finished'}
                        meta={meta || '0 scripts'}
                    >
                        <ul className="flex flex-col gap-8">
                            {scripts.map((script) => (
                                <DeliveryRow key={script.handle} script={script} />
                            ))}
                            {workers.map((update) => (
                                <WorkerUpdateRow key={update.handle} update={update} />
                            ))}
                        </ul>
                    </BackgroundReportCard>,
                    e.uid
                )
                continue
            }
            if (isBackgroundReport(content)) {
                const r = content.background_report
                const found = r.entries ?? []
                arrive(
                    <BackgroundReportCard
                        key={e.uid}
                        agentId={agent.id}
                        meta={`${found.length} piece${found.length === 1 ? '' : 's'}`}
                        linkLabel="What each piece found"
                        details={found.map((en, i) => (
                            <div key={i} className="flex flex-col gap-4">
                                <p className="type-small text-faint">
                                    {i + 1}. {en.prompt} · {en.state}
                                </p>
                                <Markdown size="document">{en.text || '_No answer._'}</Markdown>
                            </div>
                        ))}
                    />,
                    e.uid
                )
                continue
            }
            // Read mid-turn: it is drawn inside the turn that read it, between that turn's parts.
            if (steered.has(e.uid)) continue
            arrive(personMessage(e), e.uid)
            continue
        }
        const turn = byAnchor.get(e.uid)
        if (turn && (c.Response || c.Checkpoint !== undefined || c.Command !== undefined)) {
            // The finished turn settles in as one piece: the work it did and
            // what it said arrive together.
            if (c.Response || turn.blocks.length || turn.clientCalls.length || turn.questions.length || turn.pages.length) arrive(block(turn, `turn-${e.uid}`), `turn-${e.uid}`)
            if (c.Command === 'abort' && turn.aborted) arrive(<StoppedNote key={`stop-${e.uid}`}>Stopped by you at {stopTime(e)}</StoppedNote>, `stop-${e.uid}`)
        }
        if (c.Checkpoint !== undefined) {
            const handover = typeof c.Checkpoint === 'string' ? c.Checkpoint : c.Checkpoint?.handover
            // `/lobotomy` asks for exactly this: a checkpoint that carries nothing over. The
            // divider says which of the two it was, because they read the same without it.
            const kind = lobotomyAt(entries, e) ? 'lobotomy' : 'checkpoint'
            arrive(<CheckpointEntry key={`cp-${e.uid}`} handover={handover ?? null} kind={kind} />, `cp-${e.uid}`)
        }
        // A command the person ran: `/checkpoint` and `/lobotomy` are drawn as the message they
        // typed, since the divider alone does not say one was asked for. An abort already has its
        // StoppedNote.
        if (c.Command && c.Command !== 'abort' && isCommand(c.Command)) {
            arrive(
                <UserMessage key={`cmd-${e.uid}`} seed={me?.username} time={time(e.timestamp)}>
                    <Markdown size="document" slashWords>{`/${c.Command}`}</Markdown>
                </UserMessage>,
                `cmd-${e.uid}`
            )
        }
    }

    if (open && (running || open.blocks.length || open.clientCalls.length || open.questions.length || open.pages.length)) {
        items.push(
            <div key="live" className="flex flex-col gap-10">
                <AgentHeader agentId={agent.id} name={agent.name} live={agent.live} />
                {parts(open, running)}
                {/* Between rounds there is no open trail yet, but the agent is still working. */}
                {running && open.blocks[open.blocks.length - 1]?.kind !== 'trail' && <Trail trail={[]} live agentName={agent.name} />}
                {clientCalls?.(open, true)}
                {pages?.(open)}
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
                data-prompt-uid={waitingUid(q.id)}
                className="scroll-mt-24"
                queued={{ agentName: agent.name, mode: q.queue ? 'queue' : 'steer' }}
                seed={me?.username}
                attachments={q.attachments?.length ? <ReplyMedia attachments={q.attachments} /> : undefined}
            >
                <Markdown size="document" slashWords>{q.text}</Markdown>
            </UserMessage>,
            `q-${q.id}`
        )
    )
    if (error) arrive(<div key="error">{error}</div>, 'error')

    const source = useMemo(() => (topicId ? { agentId: agent.id, topicId } : null), [agent.id, topicId])

    // The prompt ruler (left edge, from tablet width up): one mark per prompt, the one being read
    // in the middle. Jumps scroll to a prompt, loading the history back to it when needed.
    const index = useMemo(() => withWaiting(promptIndex(prompts ?? [], entries), state.queued), [prompts, entries, state.queued])
    const showRuler = index.length > 0
    // Unmeasured, the newest prompt: a conversation opens at its end.
    const [measured, setCurrent] = useState<number | null>(null)
    const current = measured ?? Math.max(0, index.length - 1)
    const tops = useRef(new Map<string, number>())
    const held = useRef<string | null>(null)
    const release = useCallback(() => {
        held.current = null
    }, [])
    const measure = useCallback(() => {
        const el = scroller.current
        if (!el) return
        // The reading line: a third of the way down. At either end of the conversation it is that
        // edge of the view: at the top the first prompt on screen is the one being read, at the
        // bottom the newest.
        const box = el.getBoundingClientRect()
        const atStart = el.scrollTop < 4
        const atEnd = !atStart && el.scrollHeight - el.scrollTop - el.clientHeight < 4
        const line = box.top + (atStart ? 32 : atEnd ? el.clientHeight - 48 : el.clientHeight / 3)
        const next = new Map<string, number>()
        el.querySelectorAll<HTMLElement>('[data-prompt-uid]').forEach((n) => {
            if (n.dataset.promptUid) next.set(n.dataset.promptUid, n.getBoundingClientRect().top - line)
        })
        tops.current = next
        // A prompt jumped to stays the one being read until the person scrolls themself: where
        // the page cannot scroll it to the reading line, a neighbour would otherwise take over.
        const kept = held.current ? index.findIndex((m) => m.uid === held.current) : -1
        setCurrent(kept >= 0 ? kept : currentPrompt(index, next))
    }, [index])
    const frame = useRef(0)
    const measureSoon = useCallback(() => {
        cancelAnimationFrame(frame.current)
        frame.current = requestAnimationFrame(measure)
    }, [measure])
    useEffect(() => {
        measureSoon()
        return () => cancelAnimationFrame(frame.current)
    }, [measureSoon, entries])

    const pendingJump = useRef<string | null>(null)
    const scrollToPrompt = useCallback((uid: string) => {
        const node = scroller.current?.querySelector<HTMLElement>(`[data-prompt-uid="${CSS.escape(uid)}"]`)
        if (!node) return false
        const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
        // A long way is jumped, not scrolled: a smooth scroll over many screens takes seconds.
        const view = scroller.current
        const far = view ? Math.abs(node.getBoundingClientRect().top - view.getBoundingClientRect().top) > view.clientHeight * 3 : false
        node.scrollIntoView({ block: 'start', behavior: reduced || far ? 'auto' : 'smooth' })
        return true
    }, [])
    const jump = useCallback(
        (i: number) => {
            const mark = index[i]
            if (!mark) return
            held.current = mark.uid
            setCurrent(i)
            if (scrollToPrompt(mark.uid)) return
            pendingJump.current = mark.uid
            void onLoadThrough?.(mark)
        },
        [index, onLoadThrough, scrollToPrompt]
    )
    // A jump that had to load older history lands once the prompt is on the page.
    useEffect(() => {
        if (pendingJump.current && scrollToPrompt(pendingJump.current)) pendingJump.current = null
    }, [entries, scrollToPrompt])

    // Alt+↑/↓ step to the previous or next prompt from anywhere but a text field.
    useEffect(() => {
        if (!showRuler) return
        const onKey = (e: KeyboardEvent) => {
            if (!e.altKey || e.ctrlKey || e.metaKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return
            const t = e.target as HTMLElement | null
            if (t && (t.isContentEditable || t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT')) return
            e.preventDefault()
            // Up from inside a long reply goes to the top of the prompt it answers first.
            const top = tops.current.get(index[current]?.uid ?? '')
            const inside = top !== undefined && top < -8
            jump(e.key === 'ArrowUp' ? (inside ? current : current - 1) : current + 1)
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
    }, [showRuler, index, current, jump])

    const describe = useCallback(
        (i: number) => {
            const mark = index[i]
            return mark ? { text: mark.preview, time: mark.timestamp ? markTime(mark.timestamp) : undefined } : undefined
        },
        [index]
    )

    return (
        <EntityBroken.Provider value={broken}>
        <DetachedSource.Provider value={source}>
        <div className="relative flex min-h-0 flex-1 flex-col">
            <div
                ref={attachScroller}
                className={`flex min-h-0 flex-1 flex-col overflow-y-auto pt-24 [--list-pad:24px] ${showRuler ? 'md:[--list-pad:56px]' : ''}`}
                style={{ paddingLeft: 'var(--list-pad)', paddingRight: `calc(var(--list-pad) + ${insetRight}px)` }}
                // The person scrolling themself lets go of a prompt jumped to.
                onWheel={release}
                onTouchStart={release}
                onPointerDown={release}
                onKeyDown={release}
                onScroll={(e) => {
                    const el = e.currentTarget
                    setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < 48)
                    measureSoon()
                }}
            >
                {/* max-w-3xl: the messages sit well inside the composer deck, which is narrower than the composer. */}
                <div className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-20">
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
            {showRuler && (
                <div className="pointer-events-none absolute top-0 left-8 hidden items-center md:flex" style={{ bottom: bottomInset }}>
                    <PromptRuler className="pointer-events-auto" total={index.length} current={current} preview={describe} onJump={jump} />
                </div>
            )}
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
        </DetachedSource.Provider>
        </EntityBroken.Provider>
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

