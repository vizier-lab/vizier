import { EmptyState, Eyebrow, FilterTabs, SearchField, Sigil, StatusPanel, SubagentWorker } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import type { ChatMessage, SubagentSnapshot } from '../../api/frames'
import { CHANNEL, subagent, subagentHistory } from '../../api/endpoints/chat'
import { groupHistory, turnEvents } from '../../lib/trail'
import {
    groupHeading,
    groupWorkers,
    stopReasonText,
    workerCounts,
    workerElapsed,
    workerMeta,
    type WorkerTab,
} from '../../lib/subagents'
import { useAgents } from '../../stores/agents'
import { assetSrc } from '../../lib/fileUrls'
import { Trail } from './Trail'
import { Markdown } from '../shared/Markdown'

/** How often an expanded, unfinished worker's steps are read again: its turns stream elsewhere. */
const POLL_MS = 2000

/** Which rows a panel draws: temporary workers (spec 059) or consultations of other agents (spec 064). */
export type PanelKind = 'workers' | 'consultations'

const WORDS: Record<PanelKind, { title: string; search: string; empty: string; message: (agent: string) => string; footer: string; failed: string }> = {
    workers: {
        title: 'Subagents',
        search: 'Search workers...',
        empty: 'No subagents',
        message: (agent) => `Workers ${agent} hires for a task appear here.`,
        footer: 'Newest spawn first',
        failed: "Couldn't load this worker.",
    },
    consultations: {
        title: 'Consultations',
        search: 'Search consultations...',
        empty: 'No consultations',
        message: (agent) => `Agents ${agent} consults appear here.`,
        footer: 'Newest first',
        failed: "Couldn't load this consultation.",
    },
}

/** The consulted agent's sigil and name, for a consultation's row (frame "Consultations · open"). */
function ConsultedAgent({ agentId }: { agentId: string }) {
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    return (
        <>
            <Sigil agentId={agentId} size="sm" image={assetSrc(agent?.avatar_url)} />
            {agent?.name ?? agentId}
        </>
    )
}

/**
 * One worker or consultation in its panel: collapsed to a line, expanded to its task, its steps
 * drawn with the chat's own activity trail (read from its own conversation) and its result. No
 * stop or reply control: only the agent manages them (specs 059, 064).
 */
function WorkerEntry({
    agentId,
    topicId,
    channel,
    agentName,
    worker,
    now,
    failedText,
}: {
    agentId: string
    topicId: string
    channel: string
    agentName: string
    worker: SubagentSnapshot
    now: number
    failedText: string
}) {
    const [expanded, setExpanded] = useState(false)
    const [detail, setDetail] = useState<SubagentSnapshot | null>(null)
    const [entries, setEntries] = useState<ChatMessage[] | null>(null)
    const [failed, setFailed] = useState(false)
    const unfinished = worker.state === 'running' || worker.state === 'asking'

    useEffect(() => {
        if (!expanded) return
        let live = true
        const read = () =>
            Promise.all([
                subagent(agentId, topicId, worker.id, channel),
                subagentHistory(agentId, topicId, worker.id, channel),
            ])
                .then(([d, h]) => {
                    if (!live) return
                    setDetail(d)
                    setEntries(h ?? [])
                    setFailed(false)
                })
                .catch(() => live && setFailed(true))
        void read()
        const timer = worker.state === 'running' ? setInterval(read, POLL_MS) : undefined
        return () => {
            live = false
            if (timer) clearInterval(timer)
        }
    }, [expanded, agentId, topicId, channel, worker.id, worker.state, worker.updated_at])

    const trail = useMemo(() => (entries ? groupHistory(entries).flatMap(turnEvents) : []), [entries])
    const steps = trail.filter((e) => e.kind !== 'thought').length
    const shown = detail ?? worker

    return (
        <SubagentWorker
            task={worker.task}
            position={worker.position}
            agent={worker.consultee ? <ConsultedAgent agentId={worker.consultee} /> : undefined}
            followUps={worker.follow_ups}
            handle={worker.id}
            state={worker.state}
            agentName={agentName}
            elapsed={workerElapsed(worker, now)}
            question={worker.question ?? undefined}
            handedOver={worker.handed_over}
            stopReason={stopReasonText(worker.stop_reason, agentName)}
            expanded={expanded}
            onExpandedChange={setExpanded}
            fullTask={shown.task}
            stepCount={entries ? steps : undefined}
            steps={
                entries ? (
                    trail.length > 0 ? (
                        <Trail trail={trail} live={worker.state === 'running'} agentName={agentName} agentId={agentId} />
                    ) : (
                        <p className="type-small text-faint">No steps yet.</p>
                    )
                ) : (
                    <p className="type-small text-faint">{failed ? failedText : 'Loading...'}</p>
                )
            }
            result={!unfinished && shown.result ? <Markdown>{shown.result}</Markdown> : undefined}
            error={!unfinished && shown.error ? shown.error : undefined}
        />
    )
}

/**
 * The Subagents panel (spec 059 Story 7, frame "059 · A"): the conversation's workers, grouped by
 * the spawn that hired them. Read-only, and separate from Background work (detached scripts). With
 * `kind="consultations"` it is the Consultations panel (spec 064): the agents consulted, each row
 * led by the consulted agent.
 */
export function SubagentsPanel({
    agentId,
    topicId,
    channel = CHANNEL,
    agentName,
    workers,
    now,
    onClose,
    kind = 'workers',
}: {
    agentId: string
    topicId: string
    channel?: string
    agentName: string
    workers: SubagentSnapshot[]
    now: number
    onClose: () => void
    kind?: PanelKind
}) {
    const words = WORDS[kind]
    const [query, setQuery] = useState('')
    const [tab, setTab] = useState<WorkerTab>('running')
    const counts = workerCounts(workers)
    const groups = useMemo(() => groupWorkers(workers, query, tab), [workers, query, tab])
    const shown = groups.reduce((n, g) => n + g.workers.length, 0)

    return (
        <StatusPanel
            title={words.title}
            meta={workerMeta(workers)}
            onClose={onClose}
            search={
                <SearchField
                    value={query}
                    onValueChange={setQuery}
                    aria-label={words.search.replace('...', '')}
                    placeholder={words.search}
                    count={shown}
                />
            }
            tabs={
                <FilterTabs
                    aria-label="Filter workers by state"
                    value={tab}
                    onChange={(v) => setTab(v as WorkerTab)}
                    options={[
                        { value: 'running', label: 'Running', count: counts.running, tone: 'live' },
                        { value: 'asking', label: 'Asking', count: counts.asking },
                        { value: 'done', label: 'Done', count: counts.done },
                        { value: 'stopped', label: 'Stopped', count: counts.stopped },
                    ]}
                />
            }
            footer={<span className="type-small text-faint">{words.footer}</span>}
        >
            {groups.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon={kind === 'consultations' ? 'consult' : 'agent'}
                    title={workers.length === 0 ? words.empty : 'Nothing here'}
                    message={words.message(agentName)}
                />
            ) : (
                <div className="flex flex-col gap-12 py-6">
                    {groups.map((g) => (
                        <section key={g.id} className="flex flex-col gap-6">
                            <Eyebrow as="h3">{groupHeading(g, now, kind)}</Eyebrow>
                            <ul className="flex flex-col gap-6">
                                {g.workers.map((w) => (
                                    <li key={w.id}>
                                        <WorkerEntry
                                            agentId={agentId}
                                            topicId={topicId}
                                            channel={channel}
                                            agentName={agentName}
                                            worker={w}
                                            now={now}
                                            failedText={words.failed}
                                        />
                                    </li>
                                ))}
                            </ul>
                        </section>
                    ))}
                </div>
            )}
        </StatusPanel>
    )
}
