import { Button, Dialog, DiffView, EmptyState, FilterTabs, LoadMore, Notice, PanelHeader, RevisionRow, Skeleton } from '@vizier/design'
import { Enter, stagger } from '../lib/motion'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { coreDiff, coreHistory, coreRevision, getCore, rollbackCore, type CoreRevisionSummary } from '../api/endpoints/agentPages'
import { absoluteTime, relativeTime } from '../lib/format'
import { authorLabel, diffLines, type RevisionDiff } from '../lib/revisions'
import { Markdown } from '../screens/shared/Markdown'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { useAgents } from '../stores/agents'
import { pendingDraft } from '../stores/drafts'
import { toast } from '../stores/toasts'

const PAGE = 20

/** CORE as it reads now, or as it read at a revision, with the history beside it. Editing is its own page. */
export default function AgentCore() {
    const { agentId = '' } = useParams()
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const agentName = useAgents((s) => s.agents.find((a) => a.agent_id === agentId)?.name)

    const [core, setCore] = useState<{ content: string; seq?: number } | null>(null)
    const [history, setHistory] = useState<CoreRevisionSummary[]>([])
    const [total, setTotal] = useState(0)
    const [loadingMore, setLoadingMore] = useState(false)
    const [restoring, setRestoring] = useState<number | null>(null)
    const draft = useMemo(() => (core ? pendingDraft(`core:${agentId}`, core.content) : null), [core, agentId])

    // The revision being looked at, from the URL so it can be linked; none is the current text.
    const asked = Number(params.get('rev')) || null
    const current = history[0]
    const rev = asked !== null && asked !== current?.seq ? asked : null
    const view = params.get('view') === 'changes' ? 'changes' : 'text'
    const [old, setOld] = useState<{ seq: number; content: string } | null>(null)
    const [diff, setDiff] = useState<RevisionDiff | null>(null)

    const loadHistory = useCallback(
        async (offset: number) => {
            const page = await coreHistory(agentId, offset, PAGE)
            setTotal(page.total)
            setHistory((h) => (offset === 0 ? page.revisions : [...h, ...page.revisions]))
        },
        [agentId]
    )

    const load = useCallback(async () => {
        const c = await getCore(agentId)
        setCore({ content: c.content, seq: c.seq })
        await loadHistory(0).catch(() => {})
    }, [agentId, loadHistory])

    useEffect(() => {
        setCore(null)
        setHistory([])
        load().catch((e) => toast.error("Couldn't load CORE", e.message))
    }, [load])

    // A past revision: its text, and what it changed from the one before.
    const shown = rev ?? current?.seq ?? null
    useEffect(() => {
        setDiff(null)
        if (shown === null) return
        let live = true
        if (rev !== null) coreRevision(agentId, rev).then((r) => live && setOld(r), () => live && setOld(null))
        if (view === 'changes') {
            const previous = history.find((r) => r.seq < shown)?.seq
            coreDiff(agentId, shown, previous).then((d) => live && setDiff(d), () => live && setDiff(null))
        }
        return () => {
            live = false
        }
    }, [agentId, rev, shown, view, history])

    const choose = (seq: number | null, nextView = view) =>
        setParams(
            (p) => {
                const n = new URLSearchParams(p)
                if (seq === null || seq === current?.seq) n.delete('rev')
                else n.set('rev', String(seq))
                if (nextView === 'changes') n.set('view', 'changes')
                else n.delete('view')
                return n
            },
            { replace: true }
        )

    const restore = async (seq: number) => {
        try {
            const res = await rollbackCore(agentId, seq)
            toast.success(res.no_change ? 'CORE already matches that revision' : `Restored rev ${seq}`)
            setRestoring(null)
            choose(null)
            await load()
        } catch (e) {
            toast.error("Couldn't restore", e instanceof Error ? e.message : undefined)
        }
    }

    const meta = current
        ? `CORE.md · rev ${current.seq} · last written by ${authorLabel(current.actor, current.trigger, agentName)} · ${relativeTime(current.created_at)}`
        : 'CORE.md'
    const at = history.find((r) => r.seq === shown)
    const text = rev !== null ? (old?.seq === rev ? old.content : null) : (core?.content ?? null)

    return (
        <PageFrame
            title="Core"
            meta={meta}
            fill
            actions={
                <Button variant="primary" icon="pencil" disabled={core === null} onClick={() => navigate(`/${agentId}/core/edit`)}>
                    Edit
                </Button>
            }
        >
            <Panes
                paneLabel="History"
                pane={
                    <>
                        <PanelHeader title="History" meta={total ? `${total} revision${total === 1 ? '' : 's'}` : undefined} level={2} />
                        <div className="-mt-12 flex flex-col gap-2">
                            {core === null && <Skeleton variant="lines" lines={3} />}
                            {history.map((r, i) => (
                                <Enter key={r.seq} delay={stagger(i, 0.03)}>
                                    <RevisionRow
                                        revision={r.seq}
                                        author={authorLabel(r.actor, r.trigger, agentName)}
                                        time={relativeTime(r.created_at)}
                                        selected={shown === r.seq}
                                        onSelect={() => choose(r.seq)}
                                        trailing={i === 0 ? <span className="type-mono-xs text-faint">current</span> : undefined}
                                    />
                                </Enter>
                            ))}
                            {history.length < total && (
                                <LoadMore
                                    label="Load older"
                                    remaining={total - history.length}
                                    loading={loadingMore}
                                    onLoad={async () => {
                                        setLoadingMore(true)
                                        await loadHistory(history.length).catch(() => {})
                                        setLoadingMore(false)
                                    }}
                                />
                            )}
                        </div>
                    </>
                }
            >
                {draft && (
                    <Notice
                        tone="info"
                        title={`Unsaved edit from ${absoluteTime(draft.at)}`}
                        actions={
                            <Button size="sm" variant="secondary" onClick={() => navigate(`/${agentId}/core/edit`)}>
                                Continue editing
                            </Button>
                        }
                    >
                        {draft.stale ? 'CORE has changed since you started it.' : 'It is kept in this browser until you save or discard it.'}
                    </Notice>
                )}

                <div className="flex flex-wrap items-center gap-12">
                    <span className="flex min-w-0 flex-1 flex-col gap-2">
                        <span className="type-body-strong text-fg">{rev !== null ? `Revision ${rev}` : 'Current version'}</span>
                        {at && (
                            <span className="type-mono-xs text-faint">
                                rev {at.seq} · {authorLabel(at.actor, at.trigger, agentName)} · {absoluteTime(at.created_at)}
                            </span>
                        )}
                    </span>
                    <FilterTabs
                        aria-label="Show"
                        value={view}
                        onChange={(v) => choose(shown, v)}
                        options={[
                            { value: 'text', label: 'Text' },
                            { value: 'changes', label: 'Changes' },
                        ]}
                    />
                    {rev !== null && (
                        <>
                            <Button variant="ghost" onClick={() => choose(null)}>
                                Back to current
                            </Button>
                            <Button variant="secondary" onClick={() => navigate(`/${agentId}/core/edit?from=${rev}`)}>
                                Edit from here
                            </Button>
                            <Button variant="primary" onClick={() => setRestoring(rev)}>
                                Restore rev {rev}
                            </Button>
                        </>
                    )}
                </div>

                {view === 'changes' ? (
                    diff ? (
                        <DiffView variant="code" title={`rev ${diff.from_seq} → ${diff.to_seq}`} lines={diffLines(diff)} />
                    ) : shown === null && core ? (
                        <EmptyState kind="empty" icon="history" title="No revisions yet" message="Each save is kept here, so you can see what changed." />
                    ) : (
                        <Skeleton lines={8} />
                    )
                ) : text === null ? (
                    <Skeleton lines={12} />
                ) : text.trim() === '' ? (
                    <EmptyState
                        kind="empty"
                        icon="core"
                        title="CORE is empty"
                        message={`It is ${agentName ?? 'the agent'}’s persistent self: who it is and how it works.`}
                        action={
                            <Button variant="primary" icon="pencil" onClick={() => navigate(`/${agentId}/core/edit`)}>
                                Write it
                            </Button>
                        }
                    />
                ) : (
                    <article className="max-w-[72ch]">
                        <Markdown size="document">{text}</Markdown>
                    </article>
                )}
            </Panes>

            <Dialog
                open={restoring !== null}
                title={`Restore rev ${restoring}?`}
                message="CORE becomes that revision's text as a new revision. Nothing is lost: the current text stays in the history."
                confirmLabel="Restore"
                onConfirm={() => restoring !== null && void restore(restoring)}
                onCancel={() => setRestoring(null)}
            />
        </PageFrame>
    )
}
