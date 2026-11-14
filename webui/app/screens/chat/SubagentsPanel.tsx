import { EmptyState, Eyebrow, FilterTabs, SearchField, StatusPanel, SubagentWorker } from '@vizier/design'
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
import { Trail } from './Trail'
import { Markdown } from '../shared/Markdown'

/** How often an expanded, unfinished worker's steps are read again: its turns stream elsewhere. */
const POLL_MS = 2000

/**
 * One worker in the Subagents panel: collapsed to a line, expanded to its task, its steps drawn
 * with the chat's own activity trail (read from the worker's conversation) and its result. No stop
 * or reply control: only the main agent manages its workers (spec 059).
 */
function WorkerEntry({
    agentId,
    topicId,
    channel,
    agentName,
    worker,
    now,
}: {
    agentId: string
    topicId: string
    channel: string
    agentName: string
    worker: SubagentSnapshot
    now: number
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
                        <Trail trail={trail} live={worker.state === 'running'} agentName={agentName} />
                    ) : (
                        <p className="type-small text-faint">No steps yet.</p>
                    )
                ) : (
                    <p className="type-small text-faint">{failed ? "Couldn't load this worker." : 'Loading...'}</p>
                )
            }
            result={!unfinished && shown.result ? <Markdown>{shown.result}</Markdown> : undefined}
            error={!unfinished && shown.error ? shown.error : undefined}
        />
    )
}

/**
 * The Subagents panel (spec 059 Story 7, frame "059 · A"): the conversation's workers, grouped by
 * the spawn that hired them. Read-only, and separate from Background work (detached scripts).
 */
export function SubagentsPanel({
    agentId,
    topicId,
    channel = CHANNEL,
    agentName,
    workers,
    now,
    onClose,
}: {
    agentId: string
    topicId: string
    channel?: string
    agentName: string
    workers: SubagentSnapshot[]
    now: number
    onClose: () => void
}) {
    const [query, setQuery] = useState('')
    const [tab, setTab] = useState<WorkerTab>('all')
    const counts = workerCounts(workers)
    const groups = useMemo(() => groupWorkers(workers, query, tab), [workers, query, tab])
    const shown = groups.reduce((n, g) => n + g.workers.length, 0)

    return (
        <StatusPanel
            title="Subagents"
            meta={workerMeta(workers)}
            onClose={onClose}
            search={
                <SearchField
                    value={query}
                    onValueChange={setQuery}
                    aria-label="Search workers"
                    placeholder="Search workers..."
                    count={shown}
                />
            }
            tabs={
                <FilterTabs
                    aria-label="Filter workers by state"
                    value={tab}
                    onChange={(v) => setTab(v as WorkerTab)}
                    options={[
                        { value: 'all', label: 'All', count: counts.all },
                        { value: 'running', label: 'Running', count: counts.running, tone: 'live' },
                        { value: 'asking', label: 'Asking', count: counts.asking },
                        { value: 'done', label: 'Done', count: counts.done },
                        { value: 'stopped', label: 'Stopped', count: counts.stopped },
                    ]}
                />
            }
            footer={<span className="type-small text-faint">Newest spawn first</span>}
        >
            {groups.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="users"
                    title={workers.length === 0 ? 'No subagents' : 'Nothing here'}
                    message={`Workers ${agentName} hires for a task appear here.`}
                />
            ) : (
                <div className="flex flex-col gap-12 py-6">
                    {groups.map((g) => (
                        <section key={g.id} className="flex flex-col gap-6">
                            <Eyebrow as="h3">{groupHeading(g, now)}</Eyebrow>
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
