import { BackgroundJob, EmptyState, PanelHeader, TextLink } from '@vizier/design'
import { useEffect, useState } from 'react'
import type { BackgroundJobSnapshot } from '../../api/frames'
import { Enter, stagger } from '../../lib/motion'

const SHOWN = 5

function elapsed(job: BackgroundJobSnapshot, now: number): string {
    const end = job.finished_at ? Date.parse(job.finished_at) : now
    const s = Math.max(0, Math.round((end - Date.parse(job.created_at)) / 1000))
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

const title = (job: BackgroundJobSnapshot) =>
    job.kind === 'delegation' ? `delegate_agent → ${job.delegated_to ?? ''}` : 'paralel_subtasks'

type Props = {
    jobs: BackgroundJobSnapshot[]
    onCancel: (jobId: string) => void
    onJumpToStart: (jobId: string) => void
}

/** Background jobs launched from this topic, running ones first (spec 012). */
export function BackgroundJobsPanel({ jobs, onCancel, onJumpToStart }: Props) {
    const [all, setAll] = useState(false)
    const [now, setNow] = useState(() => Date.now())
    const running = jobs.some((j) => j.state === 'running' || j.state === 'reporting')
    useEffect(() => {
        if (!running) return
        const t = setInterval(() => setNow(Date.now()), 1000)
        return () => clearInterval(t)
    }, [running])

    const sorted = [...jobs].sort((a, b) => Number(!(a.state === 'running' || a.state === 'reporting')) - Number(!(b.state === 'running' || b.state === 'reporting')) || Date.parse(b.created_at) - Date.parse(a.created_at))
    const rows = all ? sorted : sorted.slice(0, SHOWN)
    const runningCount = jobs.filter((j) => j.state === 'running' || j.state === 'reporting').length

    return (
        <section aria-label="Background jobs" className="flex flex-col gap-8">
            <PanelHeader title="Background jobs" meta={runningCount ? `${runningCount} running` : `${jobs.length}`} level={3} />
            {jobs.length === 0 ? (
                <EmptyState kind="empty" icon="layers" title="No background jobs" message="Work the agent hands off appears here." />
            ) : (
                <>
                    {rows.map((j, i) => (
                        <Enter key={j.id} delay={stagger(i, 0.04)}>
                            <BackgroundJob
                                title={title(j)}
                                goal={j.pieces[0]?.prompt ?? ''}
                                state={j.state}
                                elapsed={elapsed(j, now)}
                                pieces={{ done: j.pieces.filter((p) => p.state !== 'running').length, total: j.pieces.length }}
                                onJumpToStart={() => onJumpToStart(j.id)}
                                onCancel={j.state === 'running' ? () => onCancel(j.id) : undefined}
                            />
                        </Enter>
                    ))}
                    {jobs.length > SHOWN && <TextLink onClick={() => setAll((a) => !a)}>{all ? 'Show fewer' : `Show all ${jobs.length}`}</TextLink>}
                </>
            )}
        </section>
    )
}
