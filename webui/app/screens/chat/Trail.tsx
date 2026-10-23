import {
    ActivityTrail,
    Fold,
    Notice,
    RunDetail,
    ScriptCall,
    Thought,
    TrailCall,
    Thinking,
} from '@vizier/design'
import { useMemo, useState } from 'react'
import { thinkingWord } from '../../lib/thinkingWords'
import type { ExecutionReport, TrailEvent } from '../../api/frames'
import { duration } from '../../lib/format'
import { trailSummary } from '../../lib/trail'
import { useMobile } from '../../lib/useMobile'

/** Past this many steps a trail shows its newest and folds the rest behind a toggle. */
const VISIBLE = 8

function counts(trail: TrailEvent[], recalled: number): string {
    const python = trail.filter((e) => e.kind === 'python').length
    const failed = trail.filter(
        (e) => e.kind === 'python' && e.report && !e.report.ok
    ).length
    const parts: string[] = []
    if (python)
        parts.push(
            `${python} python run${python === 1 ? '' : 's'}${failed ? ` (${failed} failed)` : ''}`
        )
    const tools = trail.filter((e) => e.kind === 'tool').length
    if (tools) parts.push(`${tools} tool${tools === 1 ? '' : 's'}`)
    if (recalled)
        parts.push(`${recalled} passage${recalled === 1 ? '' : 's'} recalled`)
    return parts.join(' · ')
}

function previewArgs(args: Record<string, unknown>): string {
    const first = Object.values(args).find((v) => typeof v === 'string') as
        string | undefined
    return first ? (first.length > 48 ? first.slice(0, 47) + '…' : first) : ''
}

function pythonState(
    event: Extract<TrailEvent, { kind: 'python' }>,
    live: boolean
) {
    if (event.stopped) return 'failed' as const
    if (!event.report) return live ? ('running' as const) : ('failed' as const)
    if (event.report.error?.kind === 'limit') return 'limit' as const
    return event.report.ok ? ('done' as const) : ('failed' as const)
}

/** One dot of the phone trail's summary line. */
function miniDot(
    event: TrailEvent,
    live: boolean
): 'done' | 'running' | 'failed' {
    if (event.kind === 'python') {
        const state = pythonState(event, live)
        return state === 'running'
            ? 'running'
            : state === 'done'
              ? 'done'
              : 'failed'
    }
    return event.kind === 'tool' && event.stopped ? 'failed' : 'done'
}

function Run({
    report,
    code,
    intent,
}: {
    report: ExecutionReport | null
    code: string | null
    intent: string | null
}) {
    const e = report?.error
    return (
        <RunDetail
            description={intent ?? undefined}
            code={code ?? ''}
            callCount={report?.tool_calls.length}
            output={report?.stdout || undefined}
            result={
                report && report.result != null && !e
                    ? JSON.stringify(report.result, null, 2)
                    : undefined
            }
            traceback={
                e && e.kind !== 'limit'
                    ? { where: e.kind, message: e.message, full: e.traceback }
                    : undefined
            }
            limit={
                e?.kind === 'limit'
                    ? { title: e.message, detail: e.limit ?? '' }
                    : undefined
            }
            calls={report?.tool_calls.map((c) => (
                <ScriptCall
                    key={c.seq}
                    name={c.name}
                    state={c.ok ? 'ok' : 'failed'}
                    duration={duration(c.duration_ms)}
                    args={c.arguments ? previewArgs(c.arguments) : undefined}
                />
            ))}
        />
    )
}

function Event({ event, live }: { event: TrailEvent; live: boolean }) {
    switch (event.kind) {
        case 'thought':
        case 'narration':
            return <Thought>{event.text}</Thought>
        case 'tool':
            return (
                <TrailCall
                    name={event.name}
                    state={event.stopped ? 'failed' : 'lookup'}
                    intent={previewArgs(event.args)}
                />
            )
        case 'python': {
            const state = pythonState(event, live)
            return (
                <TrailCall
                    name="execute_python"
                    intent={event.intent ?? undefined}
                    state={state}
                    duration={
                        event.report
                            ? duration(event.report.duration_ms)
                            : undefined
                    }
                >
                    {event.stopped ? (
                        <Notice compact tone="failed">
                            Stopped before it finished.
                        </Notice>
                    ) : event.report || event.code ? (
                        <Run
                            report={event.report}
                            code={event.code}
                            intent={null}
                        />
                    ) : null}
                </TrailCall>
            )
        }
    }
}

type Props = {
    trail: TrailEvent[]
    live: boolean
    /** Milliseconds the turn took, once it ended. */
    durationMs?: number
    recalled?: number
    failed?: boolean
    aborted?: boolean
    /** The agent's name, for the line shown while it works. */
    agentName?: string
}

/** The agent's working: live while it runs, folding to a summary row when it answers (FR-008b). */
export function Trail({
    trail,
    live,
    durationMs,
    recalled = 0,
    failed,
    aborted,
    agentName,
}: Props) {
    const [all, setAll] = useState(false)
    const word = useMemo(thinkingWord, [])
    const mobile = useMobile()
    if (trail.length === 0 && !live) return null
    const wait = live && (
        <Thinking>
            {agentName ? `${agentName} is ` : ''}
            {word}…
        </Thinking>
    )
    // Nothing to show yet: the dots and a word, like the legacy page.
    if (trail.length === 0) return wait

    const hidden = !all && trail.length > VISIBLE ? trail.length - VISIBLE : 0
    const shown = hidden ? trail.slice(hidden) : trail
    const title = live
        ? 'Working…'
        : failed
          ? `Failed${durationMs != null ? ` after ${duration(durationMs)}` : ''}`
          : aborted
            ? 'Stopped'
            : `Worked${durationMs != null ? ` ${duration(durationMs)}` : ''}`

    return (
        <div className="flex flex-col gap-10">
        {wait}
        <ActivityTrail
            bare
            state={live ? 'live' : failed || aborted ? 'failed' : 'done'}
            title={title}
            counts={
                counts(trail, recalled) ||
                trailSummary(trail).replace('Reasoning · ', '')
            }
            defaultExpanded={live}
            // On a phone the head carries one dot per call instead of the spine.
            mini={
                mobile
                    ? {
                          dots: trail
                              .filter(
                                  (e) =>
                                      e.kind === 'python' || e.kind === 'tool'
                              )
                              .map((e) => miniDot(e, live)),
                      }
                    : undefined
            }
        >
            {hidden > 0 && (
                <Fold
                    label={`${hidden} earlier steps`}
                    onClick={() => setAll(true)}
                />
            )}
            {all && trail.length > VISIBLE && (
                <Fold
                    label="show fewer"
                    aria-expanded
                    onClick={() => setAll(false)}
                />
            )}
            {shown.map((e) => (
                <Event key={e.id} event={e} live={live} />
            ))}
        </ActivityTrail>
        </div>
    )
}
