import {
    ActivityTrail,
    Fold,
    Notice,
    RunDetail,
    ScriptCall,
    Thought,
    TrailCall,
    TrailItem,
} from '@vizier/design'
import { useContext, useEffect, useMemo, useState } from 'react'
import { highlightCode } from '../../lib/highlight'
import { Markdown } from '../shared/Markdown'
import { useThinkingWord } from '../../lib/thinkingWords'
import type { ExecutionReport, TrailEvent } from '../../api/frames'
import { duration } from '../../lib/format'
import { miniDots, trailSummary } from '../../lib/trail'
import { useMobile } from '../../lib/useMobile'
import { detachedScript } from '../../api/endpoints/chat'
import type { DetachedScriptDetail } from '../../api/frames'
import { detachedReport, type DeliveredScript } from '../../lib/detached'
import { DetachedSource } from './detachedSource'
import type { WorkerUpdate } from '../../lib/subagents'

/** Past this many steps a trail shows its newest and folds the rest behind a toggle. */
const VISIBLE = 8

function counts(trail: TrailEvent[]): string {
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
    const delivered = trail.filter((e) => e.kind === 'delivery').length
    if (delivered)
        parts.push(`${delivered} background result${delivered === 1 ? '' : 's'}`)
    const updates = trail.filter((e) => e.kind === 'worker_update').length
    if (updates) parts.push(`${updates} subagent update${updates === 1 ? '' : 's'}`)
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
    // Detached: it handed back a handle at once, and its own outcome arrives later.
    if (event.detachedId) return 'done' as const
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
    if (event.kind === 'delivery') return deliveryState(event.state) === 'done' ? 'done' : 'failed'
    if (event.kind === 'worker_update') return workerUpdateState(event.state) === 'failed' ? 'failed' : 'done'
    return event.kind === 'tool' && event.stopped ? 'failed' : 'done'
}

/** A delivered script's dot: done on its own, failed, or stopped by its time limit. */
function deliveryState(state: string) {
    if (state === 'succeeded') return 'done' as const
    if (state === 'timed out') return 'limit' as const
    return 'failed' as const
}

/** A worker update's dot: answered, asking (amber: it waits on the agent), or failed. */
function workerUpdateState(state: string) {
    if (state === 'answered') return 'done' as const
    if (state === 'asking') return 'limit' as const
    return 'failed' as const
}

/** One worker's answer, question or failure on the trail, or in the card of an update that woke the agent. */
export function WorkerUpdateRow({ update }: { update: WorkerUpdate }) {
    return (
        <TrailCall
            name="subagent"
            intent={`${update.position} · ${update.title}`}
            state={workerUpdateState(update.state)}
            duration={`${update.handle} · ${update.state.replace('_', ' ')}`}
        >
            {update.body ? (
                <pre className="whitespace-pre-wrap break-words type-code-block text-soft">{update.body}</pre>
            ) : null}
        </TrailCall>
    )
}

/** A delivered script's own record (code, calls, result), read when its row opens. */
function DeliveredRun({ handle, body }: { handle: string; body: string }) {
    const source = useContext(DetachedSource)
    const [detail, setDetail] = useState<DetachedScriptDetail | null>(null)
    const [failed, setFailed] = useState(false)
    useEffect(() => {
        if (!source) return
        let live = true
        detachedScript(source.agentId, source.topicId, handle)
            .then((d) => live && setDetail(d))
            .catch(() => live && setFailed(true))
        return () => {
            live = false
        }
    }, [source, handle])
    if (detail) return <Run report={detachedReport(detail)} code={detail.code} intent={null} />
    if (!source || failed)
        return body ? <pre className="whitespace-pre-wrap break-words type-code-block text-soft">{body}</pre> : null
    return <p className="type-small text-faint">Loading...</p>
}

/** One delivered script on the trail, or in the card of a delivery that woke the agent. */
export function DeliveryRow({ script }: { script: DeliveredScript }) {
    return (
        <TrailCall
            name="background"
            intent={script.intent}
            state={deliveryState(script.state)}
            duration={`${script.handle} · ${script.state} in ${script.took}`}
        >
            <DeliveredRun handle={script.handle} body={script.body} />
        </TrailCall>
    )
}

export function Run({
    report,
    code,
    intent,
}: {
    report: ExecutionReport | null
    code: string | null
    intent: string | null
}) {
    const e = report?.error
    const source = code ?? ''
    const result =
        report && report.result != null && !e
            ? JSON.stringify(report.result, null, 2)
            : undefined
    // Once per run: a live trail re-renders as each step arrives, and re-highlighting a long
    // script every frame is exactly what a plain block never had to pay for.
    const codeTokens = useMemo(() => highlightCode(source, 'python'), [source])
    const resultTokens = useMemo(
        () => (result ? highlightCode(result, 'json') : undefined),
        [result]
    )
    return (
        <RunDetail
            description={intent ?? undefined}
            code={source}
            codeTokens={codeTokens}
            callCount={report?.tool_calls.length}
            output={report?.stdout || undefined}
            result={result}
            resultTokens={resultTokens}
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
            return <Thought>{event.text}</Thought>
        // Prose the agent wrote between rounds, at the point it wrote it: on the spine, where a
        // thought sits, rather than as a message that splits the turn.
        case 'narration':
            return (
                <TrailItem>
                    <Markdown size="document" tone="muted">
                        {event.text}
                    </Markdown>
                </TrailItem>
            )
        case 'tool':
            return (
                <TrailCall
                    name={event.name}
                    state={event.stopped ? 'failed' : 'lookup'}
                    intent={previewArgs(event.args)}
                />
            )
        case 'delivery':
            return <DeliveryRow script={event} />
        case 'worker_update':
            return <WorkerUpdateRow update={event} />
        case 'python': {
            const state = pythonState(event, live)
            return (
                <TrailCall
                    name="execute_python"
                    intent={event.intent ?? undefined}
                    state={state}
                    duration={
                        event.detachedId
                            ? `in the background · ${event.detachedId}`
                            : event.report
                              ? duration(event.report.duration_ms)
                              : undefined
                    }
                >
                    {event.detachedId ? (
                        <Notice compact>
                            Running in the background as {event.detachedId}. Its result arrives as a later step.
                        </Notice>
                    ) : event.stopped ? (
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
    failed?: boolean
    aborted?: boolean
    /** The agent's name, for the line shown while it works. */
    agentName?: string
    /**
     * The turn is still running, though this trail is finished: a call in it with no report yet is
     * waiting rather than failed.
     */
    turnRunning?: boolean
}

/** The agent's working: live while it runs, folding to a summary row when it answers (FR-008b). */
export function Trail({
    trail,
    live,
    durationMs,
    failed,
    aborted,
    agentName,
    turnRunning,
}: Props) {
    const [all, setAll] = useState(false)
    // The line says what the agent is doing, and changes while the turn works.
    const word = useThinkingWord(live)
    const mobile = useMobile()
    if (trail.length === 0 && !live) return null
    const hidden = !all && trail.length > VISIBLE ? trail.length - VISIBLE : 0
    const shown = hidden ? trail.slice(hidden) : trail
    const title = live
        ? `${agentName ? `${agentName} is ` : ''}${word}`
        : failed
          ? `Failed${durationMs != null ? ` after ${duration(durationMs)}` : ''}`
          : aborted
            ? 'Stopped'
            : `Worked${durationMs != null ? ` ${duration(durationMs)}` : ''}`

    return (
        <ActivityTrail
            state={live ? 'live' : failed || aborted ? 'failed' : 'done'}
            title={title}
            counts={
                counts(trail) ||
                trailSummary(trail).replace('Reasoning · ', '')
            }
            defaultExpanded={live}
            // On a phone the head carries a dot per call instead of the spine, capped so a
            // hundred calls cannot stretch the line off the screen.
            mini={
                mobile
                    ? {
                          ...miniDots(
                              trail
                                  .filter(
                                      (e) =>
                                          e.kind === 'python' ||
                                          e.kind === 'tool' ||
                                          e.kind === 'delivery' ||
                                          e.kind === 'worker_update'
                                  )
                                  .map((e) => miniDot(e, live || !!turnRunning))
                          ),
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
            {shown.map((e) => {
                // The steps stay direct children of the list: the spine's
                // not-last rule needs them as siblings. ActivityTrail animates
                // their arrival while live.
                return <Event key={e.id} event={e} live={live || !!turnRunning} />
            })}
        </ActivityTrail>
    )
}
