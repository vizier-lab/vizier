import { Button, DiffView, LoadMore, MarkdownEditor, PanelHeader, RevisionRow, UnsavedIndicator } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    coreDiff,
    coreHistory,
    coreRevision,
    getCore,
    rollbackCore,
    saveCore,
    type CoreRevisionSummary,
} from '../api/endpoints/agentPages'
import { absoluteTime, relativeTime } from '../lib/format'
import { authorLabel, diffLines, type RevisionDiff } from '../lib/revisions'
import { PageFrame } from '../screens/shared/PageFrame'
import { useAgents } from '../stores/agents'
import { clearDraft, pendingDraft, saveDraft } from '../stores/drafts'
import { toast } from '../stores/toasts'
import { Dialog } from '@vizier/design'

const PAGE = 20

export default function AgentCore() {
    const { agentId = '' } = useParams()
    const agentName = useAgents((s) => s.agents.find((a) => a.agent_id === agentId)?.name)
    const key = `core:${agentId}`

    const [saved, setSaved] = useState<{ content: string; seq?: number } | null>(null)
    const [text, setText] = useState('')
    const [saving, setSaving] = useState(false)
    const [conflict, setConflict] = useState<{ content: string; seq: number } | null>(null)
    const [draft, setDraft] = useState<ReturnType<typeof pendingDraft>>(null)

    const [history, setHistory] = useState<CoreRevisionSummary[]>([])
    const [total, setTotal] = useState(0)
    const [loadingMore, setLoadingMore] = useState(false)
    const [selected, setSelected] = useState<number | null>(null)
    const [diff, setDiff] = useState<RevisionDiff | null>(null)
    const [restoring, setRestoring] = useState<number | null>(null)

    const loadHistory = useCallback(
        async (offset: number) => {
            const page = await coreHistory(agentId, offset, PAGE)
            setTotal(page.total)
            setHistory((h) => (offset === 0 ? page.revisions : [...h, ...page.revisions]))
        },
        [agentId]
    )

    const load = useCallback(async () => {
        const core = await getCore(agentId)
        setSaved({ content: core.content, seq: core.seq })
        setText(core.content)
        setDraft(pendingDraft(key, core.content))
        await loadHistory(0).catch(() => {})
    }, [agentId, key, loadHistory])

    useEffect(() => {
        setSaved(null)
        setHistory([])
        setSelected(null)
        setDiff(null)
        load().catch((e) => toast.error("Couldn't load CORE", e.message))
    }, [load])

    const dirty = saved !== null && text !== saved.content
    const edit = (value: string) => {
        setText(value)
        if (saved) saveDraft(key, value, saved.content)
    }

    const save = useCallback(async () => {
        if (!saved || saving || !dirty) return
        setSaving(true)
        try {
            const res = await saveCore(agentId, text, saved.seq)
            clearDraft(key)
            setSaved({ content: text, seq: res?.seq ?? saved.seq })
            toast.success('CORE saved')
            await loadHistory(0).catch(() => {})
        } catch (e) {
            if (e instanceof ApiError && e.status === 409) {
                const data = e.data as { latest_seq: number; content: string }
                setConflict({ content: data.content, seq: data.latest_seq })
            } else toast.error("Couldn't save CORE", e instanceof Error ? e.message : undefined)
        } finally {
            setSaving(false)
        }
    }, [agentId, dirty, key, loadHistory, saved, saving, text])

    // "Keep mine" saves over theirs, now knowing which revision that is; "Reload theirs" drops the edit.
    const resolveConflict = async (keepMine: boolean) => {
        if (!conflict) return
        if (keepMine) {
            setSaved({ content: conflict.content, seq: conflict.seq })
            setConflict(null)
            setSaving(true)
            try {
                const res = await saveCore(agentId, text, conflict.seq)
                clearDraft(key)
                setSaved({ content: text, seq: res?.seq ?? conflict.seq })
                toast.success('CORE saved')
                await loadHistory(0).catch(() => {})
            } catch (e) {
                toast.error("Couldn't save CORE", e instanceof Error ? e.message : undefined)
            } finally {
                setSaving(false)
            }
        } else {
            clearDraft(key)
            setSaved({ content: conflict.content, seq: conflict.seq })
            setText(conflict.content)
            setConflict(null)
            await loadHistory(0).catch(() => {})
        }
    }

    const open = async (seq: number) => {
        setSelected(seq)
        setDiff(null)
        const previous = history.find((r) => r.seq < seq)?.seq
        try {
            setDiff(await coreDiff(agentId, seq, previous))
        } catch (e) {
            toast.error("Couldn't load that revision", e instanceof Error ? e.message : undefined)
        }
    }

    const restore = async (seq: number) => {
        try {
            const res = await rollbackCore(agentId, seq)
            toast.success(res.no_change ? 'CORE already matches that revision' : `Restored rev ${seq}`)
            setRestoring(null)
            setSelected(null)
            setDiff(null)
            await load()
        } catch (e) {
            toast.error("Couldn't restore", e instanceof Error ? e.message : undefined)
        }
    }

    const current = history[0]
    const meta = useMemo(
        () =>
            current
                ? `CORE.md · rev ${current.seq} · last written by ${authorLabel(current.actor, current.trigger, agentName)} · ${relativeTime(current.created_at)}`
                : 'CORE.md',
        [current, agentName]
    )

    const preview = async (seq: number) => {
        const rev = await coreRevision(agentId, seq)
        setText(rev.content)
        setSelected(null)
    }

    return (
        <PageFrame
            title="Core"
            meta={meta}
            width="wide"
            actions={
                <div className="flex items-center gap-12">
                    {dirty && <UnsavedIndicator>Unsaved · draft kept in this browser</UnsavedIndicator>}
                    <Button variant="ghost" disabled={!dirty} onClick={() => (clearDraft(key), setText(saved?.content ?? ''))}>
                        Discard
                    </Button>
                    <Button variant="primary" loading={saving} disabled={!dirty} shortcut="Ctrl S" onClick={() => void save()}>
                        Save
                    </Button>
                </div>
            }
        >
            {draft && !dirty && (
                <div className="flex items-center gap-12 rounded-lg border border-border-strong bg-surface px-16 py-12 type-small text-muted">
                    <span className="flex-1">
                        You have an unsaved draft from {absoluteTime(draft.at)}.{draft.stale ? ' CORE has changed since, so restoring it would replace those changes.' : ''}
                    </span>
                    <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => {
                            setText(draft.text)
                            setDraft(null)
                        }}
                    >
                        Restore draft
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => (clearDraft(key), setDraft(null))}>
                        Discard
                    </Button>
                </div>
            )}

            <MarkdownEditor look="compact" value={text} onChange={edit} onSaveShortcut={() => void save()} className="h-640" />

            <section aria-label="History" className="flex flex-col gap-8">
                <PanelHeader title="History" meta={total ? `${total} revision${total === 1 ? '' : 's'}` : undefined} level={2} />
                {history.map((r, i) => (
                    <RevisionRow
                        key={r.seq}
                        revision={r.seq}
                        author={authorLabel(r.actor, r.trigger, agentName)}
                        time={relativeTime(r.created_at)}
                        selected={selected === r.seq}
                        onSelect={() => void open(r.seq)}
                        trailing={i === 0 ? <span className="type-mono-xs text-faint">current</span> : undefined}
                    />
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
                {selected !== null && diff && (
                    <DiffView
                        title={`rev ${diff.from_seq} → ${diff.to_seq}`}
                        lines={diffLines(diff)}
                        restoreLabel={selected !== current?.seq ? `Restore rev ${selected}` : undefined}
                        onRestore={selected !== current?.seq ? () => setRestoring(selected) : undefined}
                    />
                )}
                {selected !== null && selected !== current?.seq && (
                    <div>
                        <Button size="sm" variant="ghost" onClick={() => void preview(selected)}>
                            Load rev {selected} into the editor
                        </Button>
                    </div>
                )}
            </section>

            <Dialog
                open={restoring !== null}
                title={`Restore rev ${restoring}?`}
                message="CORE becomes that revision's text as a new revision. Nothing is lost: the current text stays in the history."
                confirmLabel="Restore"
                onConfirm={() => restoring !== null && void restore(restoring)}
                onCancel={() => setRestoring(null)}
            />

            <Dialog
                open={conflict !== null}
                title="CORE changed while you were editing"
                message={`Someone saved revision ${conflict?.seq ?? ''} after you opened it. Reload theirs to drop your edit, or keep yours to save it over theirs as a new revision.`}
                confirmLabel="Keep mine"
                cancelLabel="Reload theirs"
                onConfirm={() => void resolveConflict(true)}
                onCancel={() => void resolveConflict(false)}
            />
        </PageFrame>
    )
}
