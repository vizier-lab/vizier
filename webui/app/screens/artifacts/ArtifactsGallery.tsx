import { ActivityItem, Button, EmptyState, FileTypeTag, LoadMore, PanelHeader, SearchField, Skeleton, SortSelect, Table, TableRow, TextField } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
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
import { absoluteTime, relativeTime } from '../../lib/format'
import { PageFrame } from '../shared/PageFrame'
import { Panes } from '../shared/Panes'
import { ConfirmDialog } from '../shared/ConfirmDialog'
import { Enter, stagger } from '../../lib/motion'
import { toast } from '../../stores/toasts'
import { FilePreviewModal, type PreviewFile } from '../shared/FilePreviewModal'
import { ArtifactDetail } from './ArtifactDetail'

const toPreview = (a: ArtifactItem): PreviewFile => ({
    key: a.id,
    name: a.name,
    mimeType: a.mime_type,
    size: a.size,
    url: `/agents/${a.agent_id}/artifacts/${encodeURIComponent(a.id)}/content`,
    meta: `${bytesLabel(a.size)} · ${kindLabel(a.kind)} · v${a.version}`,
})

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
    const [search, setSearch] = useState('')
    const [q, setQ] = useState('')
    const [kind, setKind] = useState<ArtifactKind | 'all'>('all')
    const [from, setFrom] = useState('')
    const [to, setTo] = useState('')
    const [items, setItems] = useState<ArtifactItem[]>([])
    const [next, setNext] = useState<string | null>(null)
    const [stats, setStats] = useState({ total: 0, bytes: 0, agentTotal: 0, agentBytes: 0 })
    const [activity, setActivity] = useState<ArtifactEvent[]>([])
    const [activityLoading, setActivityLoading] = useState(true)
    const [activityNext, setActivityNext] = useState<string | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)
    const [detail, setDetail] = useState<Detail | null>(null)
    const [detailOpen, setDetailOpen] = useState(false)
    const [preview, setPreview] = useState<number | null>(null)
    const [confirm, setConfirm] = useState<ArtifactItem | null>(null)
    const [deleting, setDeleting] = useState(false)

    const manage = agent ? canManageArtifacts(agent) : false

    useEffect(() => {
        if (!agentId) return
        listAgents()
            .then((list) => setAgent(list.find((a) => a.agent_id === agentId) ?? null))
            .catch(() => setAgent(null))
    }, [agentId])

    // Typing narrows the list a moment after the last key, not on every one.
    useEffect(() => {
        if (search === q) return
        const t = setTimeout(() => setQ(search), 250)
        return () => clearTimeout(t)
    }, [search, q])

    const loadFirst = useCallback(async () => {
        setLoading(true)
        setError(null)
        try {
            const page = await listArtifacts(agentId, { q, kind, from, to })
            setItems(page.items)
            setNext(page.next ?? null)
            setStats({
                total: page.total,
                bytes: page.total_bytes,
                agentTotal: page.agent_total,
                agentBytes: page.agent_total_bytes,
            })
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Could not load artifacts')
        } finally {
            setLoading(false)
        }
    }, [agentId, q, kind, from, to])

    const loadActivity = useCallback(async () => {
        try {
            const page = await artifactActivity(agentId, undefined, 20)
            setActivity(page.items)
            setActivityNext(page.next ?? null)
        } catch {
            setActivity([])
        } finally {
            setActivityLoading(false)
        }
    }, [agentId])

    useEffect(() => {
        void loadFirst()
    }, [loadFirst])

    useEffect(() => {
        void loadActivity()
    }, [loadActivity])

    const loadMore = async () => {
        if (!next) return
        try {
            const page = await listArtifacts(agentId, { q, kind, from, to, cursor: next })
            setItems((prev) => [...prev, ...page.items])
            setNext(page.next ?? null)
        } catch (e) {
            toast.error("Couldn't load more", e instanceof Error ? e.message : undefined)
        }
    }

    const loadMoreActivity = async () => {
        if (!activityNext) return
        try {
            const page = await artifactActivity(agentId, activityNext, 20)
            setActivity((prev) => [...prev, ...page.items])
            setActivityNext(page.next ?? null)
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
            await Promise.all([loadFirst(), loadActivity()])
        } catch (e) {
            toast.error("Couldn't delete the artifact", e instanceof Error ? e.message : undefined)
        } finally {
            setDeleting(false)
        }
    }

    const clearFilters = () => {
        setSearch('')
        setQ('')
        setKind('all')
        setFrom('')
        setTo('')
    }

    const filtered = q !== '' || kind !== 'all' || from !== '' || to !== ''
    const forbidden = !!agent && !canViewArtifacts(agent)

    return (
        <PageFrame
            fill={!forbidden}
            title="Artifacts"
            meta={`Permanent files this agent kept · ${plural(stats.agentTotal, 'artifact')} · ${bytesLabel(stats.agentBytes)}`}
        >
            {forbidden ? (
                <EmptyState
                    kind="error"
                    icon="lock"
                    title="You can’t view this agent’s artifacts"
                    message="Artifacts require the “View every agent’s artifacts” or “View artifacts of agents they own” permission."
                />
            ) : (
                <Panes paneLabel="Recent activity" pane={<ActivityPane events={activity} loading={activityLoading} more={activityNext !== null} onMore={() => void loadMoreActivity()} />}>
                    <div className="flex flex-col gap-16">
                        {(items.length > 0 || filtered) && (
                            <div className="flex flex-wrap items-center gap-12">
                                <div className="min-w-180 max-w-md flex-1">
                                    <SearchField
                                        aria-label="Search artifacts by name"
                                        placeholder="Search artifacts by name"
                                        value={search}
                                        onValueChange={setSearch}
                                        count={filtered ? `${stats.total} of ${stats.agentTotal}` : undefined}
                                    />
                                </div>
                                <SortSelect
                                    label="Kind"
                                    value={kind}
                                    options={KINDS.map((k) => ({ value: k, label: k === 'all' ? 'All' : kindLabel(k) }))}
                                    onChange={(v) => setKind(v as ArtifactKind | 'all')}
                                />
                                <div className="w-170">
                                    <TextField aria-label="Changed from" type="date" suffix="from" value={from} onChange={(e) => setFrom(e.target.value)} />
                                </div>
                                <div className="w-170">
                                    <TextField aria-label="Changed until" type="date" suffix="to" value={to} onChange={(e) => setTo(e.target.value)} />
                                </div>
                                {filtered && (
                                    <Button variant="ghost" onClick={clearFilters}>
                                        Clear filters
                                    </Button>
                                )}
                            </div>
                        )}

                        {error ? (
                            <EmptyState
                                kind="error"
                                title="Couldn't load artifacts"
                                message={error}
                                action={<Button onClick={() => void loadFirst()}>Retry</Button>}
                            />
                        ) : loading && items.length === 0 ? (
                            <Skeleton lines={4} />
                        ) : items.length === 0 ? (
                            filtered ? (
                                <EmptyState
                                    kind="no-results"
                                    title="Nothing matched"
                                    message="Try a different search or filter."
                                    action={<Button onClick={clearFilters}>Clear filters</Button>}
                                />
                            ) : (
                                <EmptyState
                                    kind="empty"
                                    icon="folder"
                                    title="No artifacts yet"
                                    message="Promote a file from a conversation to keep it."
                                />
                            )
                        ) : (
                            <div aria-busy={loading} className="flex flex-col gap-8">
                                <Table
                                    label="Artifacts"
                                    columns={[
                                        { label: 'Artifact' },
                                        { label: 'Size', width: 100 },
                                        { label: 'Changed', width: 130 },
                                        { label: '', width: 220, align: 'end', srLabel: 'Actions' },
                                    ]}
                                >
                                    {items.map((item) => (
                                        <TableRow
                                            key={item.id}
                                            endAligned={[3]}
                                            onSelect={() => setPreview(items.indexOf(item))}
                                            cells={[
                                                <span key="n" className="flex min-w-0 items-center gap-10">
                                                    <FileTypeTag file={item.name} />
                                                    <span className="flex min-w-0 flex-col">
                                                        <span data-title className="truncate type-code text-fg">
                                                            {item.name}
                                                        </span>
                                                        <span className={`truncate type-small ${item.available ? 'text-faint' : 'text-failed-text'}`}>
                                                            {item.available ? `${kindLabel(item.kind)} · v${item.version}` : unavailableLabel(item)}
                                                        </span>
                                                    </span>
                                                </span>,
                                                <span key="s" className="type-small text-muted">{bytesLabel(item.size)}</span>,
                                                <time key="u" className="type-small text-muted" dateTime={item.updated_at} title={absoluteTime(item.updated_at)}>
                                                    {relativeTime(item.updated_at)}
                                                </time>,
                                                <span key="a" className="inline-flex justify-end gap-4" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                                                    <Button variant="ghost" icon="info" aria-label={`Details of ${item.name}`} onClick={() => void openDetail(item)}>
                                                        Details
                                                    </Button>
                                                    {manage && (
                                                        <Button variant="ghost" icon="trash" aria-label={`Delete ${item.name}`} onClick={() => setConfirm(item)}>
                                                            Delete
                                                        </Button>
                                                    )}
                                                </span>,
                                            ]}
                                        />
                                    ))}
                                </Table>
                                {next && <LoadMore label="Load more" remaining={Math.max(stats.total - items.length, 0)} onLoad={() => void loadMore()} />}
                            </div>
                        )}
                    </div>
                </Panes>
            )}

            <ArtifactDetail
                detail={detail}
                open={detailOpen}
                canManage={manage}
                onClose={() => setDetailOpen(false)}
                onDelete={() => {
                    if (detail) setConfirm(detail.artifact)
                }}
                onPreview={() => {
                    const i = detail ? items.findIndex((a) => a.id === detail.artifact.id) : -1
                    if (i >= 0) {
                        setDetailOpen(false)
                        setPreview(i)
                    }
                }}
            />
            {preview !== null && items[preview] && (
                <FilePreviewModal files={items.map(toPreview)} index={preview} onIndex={setPreview} onClose={() => setPreview(null)} />
            )}
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

/** The artifact store's change record: who promoted, replaced or deleted which file. */
function ActivityPane({ events, loading, more, onMore }: { events: ArtifactEvent[]; loading: boolean; more: boolean; onMore: () => void }) {
    return (
        <section aria-label="Recent activity" className="flex flex-col gap-12">
            <PanelHeader title="Recent activity" level={2} />
            {loading ? (
                <Skeleton variant="lines" lines={2} />
            ) : events.length === 0 ? (
                <p className="type-small text-faint">Nothing has changed yet. Every promotion, replacement and deletion is recorded here, with who made it.</p>
            ) : (
                <div className="flex flex-col">
                    {events.map((event, i) => (
                        <Enter key={event.id} delay={stagger(i, 0.03)}>
                            <ActivityItem
                                actor={event.actor_name}
                                time={
                                    <time dateTime={event.created_at} title={absoluteTime(event.created_at)}>
                                        {relativeTime(event.created_at)}
                                    </time>
                                }
                                text={
                                    <>
                                        {event.action} <span className="type-code">{event.artifact_name}</span>
                                    </>
                                }
                            />
                        </Enter>
                    ))}
                    {more && <LoadMore label="Load older" onLoad={onMore} />}
                </div>
            )}
        </section>
    )
}
