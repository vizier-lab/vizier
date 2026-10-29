import { Button, EmptyState, FilterTabs } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { listAgents } from '../../api/endpoints/agents'
import {
    artifactActivity,
    deleteArtifact,
    getArtifact,
    listArtifacts,
    type ArtifactDetail as Detail,
    type ArtifactEvent,
    type ArtifactItem,
    type ArtifactKind,
} from '../../api/endpoints/artifacts'
import { canManageArtifacts, canViewArtifacts } from '../../lib/access'
import { bytesLabel, kindLabel, plural, unavailableLabel } from '../../lib/artifacts'
import { relativeTime } from '../../lib/format'
import { PageFrame } from '../shared/PageFrame'
import { ConfirmDialog } from '../shared/ConfirmDialog'
import { toast } from '../../stores/toasts'
import { ArtifactDetail } from './ArtifactDetail'
import { ArtifactPreviewDialog } from './ArtifactPreviewDialog'

const KINDS: (ArtifactKind | 'all')[] = [
    'all',
    'image',
    'pdf',
    'text',
    'markdown',
    'csv',
    'audio',
    'video',
    'other',
]

export function ArtifactsGallery() {
    const { agentId = '' } = useParams()
    const [agent, setAgent] = useState<{ owner_id?: string } | null>(null)
    const [tab, setTab] = useState<'artifacts' | 'activity'>('artifacts')
    const [q, setQ] = useState('')
    const [kind, setKind] = useState<ArtifactKind | 'all'>('all')
    const [from, setFrom] = useState('')
    const [to, setTo] = useState('')
    const [items, setItems] = useState<ArtifactItem[]>([])
    const [next, setNext] = useState<string | null>(null)
    const [stats, setStats] = useState({ total: 0, bytes: 0, agentTotal: 0, agentBytes: 0 })
    const [activity, setActivity] = useState<ArtifactEvent[]>([])
    const [activityNext, setActivityNext] = useState<string | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)
    const [detail, setDetail] = useState<Detail | null>(null)
    const [detailOpen, setDetailOpen] = useState(false)
    const [preview, setPreview] = useState<ArtifactItem | null>(null)
    const [previewOpen, setPreviewOpen] = useState(false)
    const [confirm, setConfirm] = useState<ArtifactItem | null>(null)
    const [deleting, setDeleting] = useState(false)

    const manage = agent ? canManageArtifacts(agent) : false

    useEffect(() => {
        if (!agentId) return
        listAgents()
            .then((list) => setAgent(list.find((a) => a.agent_id === agentId) ?? null))
            .catch(() => setAgent(null))
    }, [agentId])

    const loadFirst = useCallback(async () => {
        setLoading(true)
        setError(null)
        try {
            if (tab === 'activity') {
                const page = await artifactActivity(agentId)
                setActivity(page.items)
                setActivityNext(page.next ?? null)
            } else {
                const page = await listArtifacts(agentId, { q, kind, from, to })
                setItems(page.items)
                setNext(page.next ?? null)
                setStats({
                    total: page.total,
                    bytes: page.total_bytes,
                    agentTotal: page.agent_total,
                    agentBytes: page.agent_total_bytes,
                })
            }
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Could not load artifacts')
        } finally {
            setLoading(false)
        }
    }, [agentId, tab, q, kind, from, to])

    useEffect(() => {
        void loadFirst()
    }, [loadFirst])

    const loadMore = async () => {
        try {
            if (tab === 'activity') {
                if (!activityNext) return
                const page = await artifactActivity(agentId, activityNext)
                setActivity((prev) => [...prev, ...page.items])
                setActivityNext(page.next ?? null)
            } else {
                if (!next) return
                const page = await listArtifacts(agentId, { q, kind, from, to, cursor: next })
                setItems((prev) => [...prev, ...page.items])
                setNext(page.next ?? null)
            }
        } catch (e) {
            toast.error("Couldn't load more", e instanceof Error ? e.message : undefined)
        }
    }

    const openDetail = async (item: ArtifactItem) => {
        try {
            setDetail(await getArtifact(agentId, item.id))
            setDetailOpen(true)
        } catch (e) {
            toast.error("Couldn't open the artifact", e instanceof Error ? e.message : undefined)
        }
    }

    const doDelete = async () => {
        if (!confirm) return
        setDeleting(true)
        try {
            await deleteArtifact(agentId, confirm.id)
            toast.success(`Deleted “${confirm.name}”`)
            setConfirm(null)
            setDetailOpen(false)
            await loadFirst()
        } catch (e) {
            toast.error("Couldn't delete the artifact", e instanceof Error ? e.message : undefined)
        } finally {
            setDeleting(false)
        }
    }

    const clearFilters = () => {
        setQ('')
        setKind('all')
        setFrom('')
        setTo('')
    }

    const filtered = q !== '' || kind !== 'all' || from !== '' || to !== ''
    const header = useMemo(
        () => (
            <span className="type-small text-muted">
                {plural(stats.agentTotal, 'artifact')} · {bytesLabel(stats.agentBytes)}
            </span>
        ),
        [stats.agentTotal, stats.agentBytes]
    )

    return (
        <PageFrame
            title="Artifacts"
            description="Permanent files this agent kept."
            meta={header}
        >
            <div className="flex flex-col gap-16 px-16 py-16 sm:px-24">
                {!agent || canViewArtifacts(agent) || !agent ? null : (
                    <p role="alert" className="type-ui-para text-failed">
                        You don't have permission to view this agent's artifacts.
                    </p>
                )}
                <div className="flex flex-wrap items-center gap-10">
                    <FilterTabs
                        aria-label="Artifacts or activity"
                        value={tab}
                        onChange={setTab}
                        options={[
                            { value: 'artifacts', label: 'Artifacts' },
                            { value: 'activity', label: 'Activity' },
                        ]}
                    />
                    {tab === 'artifacts' && (
                        <>
                            <input
                                aria-label="Search artifacts by name"
                                placeholder="Search by name"
                                value={q}
                                onChange={(e) => setQ(e.target.value)}
                                className="h-34 min-w-180 rounded-md border border-border-strong bg-surface px-10 type-small text-fg"
                            />
                            <select
                                aria-label="Filter by kind"
                                value={kind}
                                onChange={(e) => setKind(e.target.value as ArtifactKind | 'all')}
                                className="h-34 rounded-md border border-border-strong bg-surface px-8 type-small text-fg"
                            >
                                {KINDS.map((k) => (
                                    <option key={k} value={k}>
                                        {k === 'all' ? 'All kinds' : kindLabel(k)}
                                    </option>
                                ))}
                            </select>
                            <label className="flex items-center gap-6 type-small text-muted">
                                From
                                <input
                                    type="date"
                                    value={from}
                                    onChange={(e) => setFrom(e.target.value)}
                                    className="h-34 rounded-md border border-border-strong bg-surface px-8 type-small text-fg"
                                />
                            </label>
                            <label className="flex items-center gap-6 type-small text-muted">
                                To
                                <input
                                    type="date"
                                    value={to}
                                    onChange={(e) => setTo(e.target.value)}
                                    className="h-34 rounded-md border border-border-strong bg-surface px-8 type-small text-fg"
                                />
                            </label>
                            {filtered && <Button onClick={clearFilters}>Clear filters</Button>}
                        </>
                    )}
                </div>

                {loading ? (
                    <p role="status" className="type-ui-para text-muted">
                        Loading artifacts…
                    </p>
                ) : error ? (
                    <EmptyState
                        kind="error"
                        title="Couldn't load artifacts"
                        message={error}
                        action={<Button onClick={() => void loadFirst()}>Retry</Button>}
                    />
                ) : tab === 'artifacts' ? (
                    items.length === 0 ? (
                        filtered ? (
                            <EmptyState
                                kind="no-results"
                                title="No artifacts match these filters."
                                action={<Button onClick={clearFilters}>Clear filters</Button>}
                            />
                        ) : (
                            <EmptyState
                                icon="folder"
                                title="No artifacts yet."
                                message="Promote a file from a conversation to keep it."
                            />
                        )
                    ) : (
                        <div className="flex flex-col gap-8">
                            <p className="type-small text-muted">
                                {plural(stats.total, 'artifact')} · {bytesLabel(stats.bytes)}
                            </p>
                            <ul aria-label="Artifacts" className="flex flex-col divide-y divide-border rounded-lg border border-border">
                                {items.map((item) => (
                                    <li key={item.id} className="flex flex-wrap items-center gap-12 px-12 py-10">
                                        <button
                                            type="button"
                                            onClick={() => void openDetail(item)}
                                            className="min-w-0 flex-1 text-left"
                                        >
                                            <span className="block truncate type-code-medium text-fg">{item.name}</span>
                                            <span className="block truncate type-small text-muted">
                                                {kindLabel(item.kind)} · {bytesLabel(item.size)} · v{item.version} ·{' '}
                                                {relativeTime(Date.parse(item.updated_at))}
                                            </span>
                                            {!item.available && (
                                                <span className="block type-small text-failed" role="status">
                                                    {unavailableLabel(item)}
                                                </span>
                                            )}
                                        </button>
                                        <Button onClick={() => { setPreview(item); setPreviewOpen(true) }}>Preview</Button>
                                        <Button onClick={() => void openDetail(item)}>Open</Button>
                                        {manage && (
                                            <Button variant="destructive" onClick={() => setConfirm(item)}>
                                                Delete
                                            </Button>
                                        )}
                                    </li>
                                ))}
                            </ul>
                            {next && <Button onClick={() => void loadMore()}>Load more</Button>}
                        </div>
                    )
                ) : activity.length === 0 ? (
                    <EmptyState icon="folder" title="No artifact activity yet." />
                ) : (
                    <div className="flex flex-col gap-8">
                        <ul aria-label="Artifact activity" className="flex flex-col divide-y divide-border rounded-lg border border-border">
                            {activity.map((event) => (
                                <li key={event.id} className="flex flex-col gap-2 px-12 py-10">
                                    <span className="type-body-strong text-fg">{event.artifact_name}</span>
                                    <span className="type-small text-muted">
                                        {event.action} by {event.actor_name} · {relativeTime(Date.parse(event.created_at))}
                                        {event.source_filename ? ` · ${event.source_filename}` : ''}
                                    </span>
                                </li>
                            ))}
                        </ul>
                        {activityNext && <Button onClick={() => void loadMore()}>Load more</Button>}
                    </div>
                )}
            </div>

            <ArtifactDetail
                detail={detail}
                open={detailOpen}
                canManage={manage}
                onClose={() => setDetailOpen(false)}
                onDelete={() => {
                    if (detail) setConfirm(detail.artifact)
                }}
                onPreview={() => {
                    if (detail) {
                        setPreview(detail.artifact)
                        setPreviewOpen(true)
                    }
                }}
            />
            <ArtifactPreviewDialog artifact={preview} open={previewOpen} onClose={() => setPreviewOpen(false)} />
            <ConfirmDialog
                open={confirm !== null}
                title={`Delete “${confirm?.name ?? ''}”?`}
                removes={[`${plural(detail?.references.length ?? 0, 'reference')} will show it unavailable`]}
                message="Messages will still show the attachment as unavailable, and memories will lose the link. The file itself is removed for good."
                confirmLabel="Delete"
                busy={deleting}
                onConfirm={() => void doDelete()}
                onCancel={() => setConfirm(null)}
            />
        </PageFrame>
    )
}
