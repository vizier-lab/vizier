import { Badge, Button, DiffView, EmptyState, Eyebrow, FilterTabs, Icon, KeyValueList, LoadMore, MarkdownEditor, Notice, PanelHeader, RevisionRow, Skeleton, UnsavedIndicator, type DocLink } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link, useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    bundleGraph,
    deleteDoc,
    getDoc,
    memoryDiff,
    memoryHistory,
    memoryRevision,
    rollbackDoc,
    saveDoc,
    type GraphDto,
    type MemoryDoc,
    type MemoryRevisionSummary,
} from '../api/endpoints/memory'
import { absoluteTime, relativeTime } from '../lib/format'
import { linkWikiLinks, relationLink } from '../lib/memoryLinks'
import { bundleHref, chatFrom, docFromChat, docHref } from '../lib/memoryTree'
import { authorLabel, diffLines, type RevisionDiff } from '../lib/revisions'
import { useMobile } from '../lib/useMobile'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { Markdown } from '../screens/shared/Markdown'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { useAgents } from '../stores/agents'
import { clearDraft, pendingDraft, saveDraft } from '../stores/drafts'
import { toast } from '../stores/toasts'

/**
 * One memory document on its own page. Reading it, with what it links and its history beside it;
 * `?edit=1` turns the page into its editor, `?rev=N` shows a past revision.
 */
export default function MemoryDoc() {
    const { agentId = '', bundle: rawBundle = '', '*': rawPath = '' } = useParams()
    const bundle = decodeURIComponent(rawBundle)
    const path = rawPath.split('/').map(decodeURIComponent).join('/').replace(/\.md$/, '')
    const [params] = useSearchParams()
    const key = `${agentId}/${bundle}/${path}`
    return params.get('edit') ? <EditDoc key={key} agentId={agentId} bundle={bundle} path={path} /> : <ReadDoc key={key} agentId={agentId} bundle={bundle} path={path} />
}

type DocProps = { agentId: string; bundle: string; path: string }

function ReadDoc({ agentId, bundle, path }: DocProps) {
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const agentName = useAgents((s) => s.agents.find((a) => a.agent_id === agentId)?.name)
    const [doc, setDoc] = useState<MemoryDoc | null>(null)
    const [missing, setMissing] = useState(false)
    const [graph, setGraph] = useState<GraphDto | null>(null)
    const [history, setHistory] = useState<MemoryRevisionSummary[]>([])
    const [total, setTotal] = useState(0)
    const [deleting, setDeleting] = useState(false)
    const [restoring, setRestoring] = useState<number | null>(null)
    const draft = useMemo(() => (doc ? pendingDraft(`memory:${agentId}:${bundle}/${path}`, doc.content) : null), [doc, agentId, bundle, path])

    const loadHistory = useCallback(
        async (offset: number) => {
            const page = await memoryHistory(agentId, bundle, path, offset)
            setTotal(page.total)
            setHistory((h) => (offset === 0 ? page.revisions : [...h, ...page.revisions]))
        },
        [agentId, bundle, path]
    )

    const load = useCallback(async () => {
        try {
            setDoc(await getDoc(agentId, bundle, path))
        } catch (e) {
            if (e instanceof ApiError && e.status === 404) setMissing(true)
            else toast.error("Couldn't load the document", e instanceof Error ? e.message : undefined)
        }
        await loadHistory(0).catch(() => {})
    }, [agentId, bundle, path, loadHistory])

    useEffect(() => void load(), [load])
    useEffect(() => {
        bundleGraph(agentId, bundle).then(setGraph, () => setGraph(null))
    }, [agentId, bundle])

    const current = history[0]
    const asked = Number(params.get('rev')) || null
    const rev = asked !== null && asked !== current?.seq ? asked : null
    const view = params.get('view') === 'changes' ? 'changes' : 'text'
    const shown = rev ?? current?.seq ?? null
    const [diff, setDiff] = useState<RevisionDiff | null>(null)
    const [old, setOld] = useState<{ seq: number; content: string } | null>(null)

    useEffect(() => {
        setDiff(null)
        if (shown === null) return
        let live = true
        const previous = history.find((r) => r.seq < shown)?.seq
        if (rev !== null && view === 'text')
            memoryRevision(agentId, bundle, path, rev).then(
                // A snapshot carries its frontmatter; the page shows the body, as it does the current text.
                (r) => live && setOld({ seq: rev, content: (r.content ?? '').replace(/^---\n[\s\S]*?\n---\n?/, '') }),
                () => live && setOld(null)
            )
        if (view === 'changes') memoryDiff(agentId, bundle, path, shown, previous).then((d) => live && setDiff(d), () => live && setDiff(null))
        return () => {
            live = false
        }
    }, [agentId, bundle, path, rev, shown, view, history])

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

    const linkedFrom = useMemo<DocLink[]>(() => {
        if (!graph) return []
        const here = graph.nodes.find((n) => n.slug === path && n.bundle === bundle && !n.boundary)
        if (!here) return []
        return graph.edges
            .filter((e) => e.target === here.slug && !e.broken)
            .map((e) => graph.nodes.find((n) => n.slug === e.source))
            .filter((n): n is NonNullable<typeof n> => !!n && !n.boundary)
            .map((n) => ({ label: n.title, href: docHref(agentId, n.bundle, n.slug) }))
    }, [graph, path, bundle, agentId])

    const restore = async (seq: number) => {
        try {
            await rollbackDoc(agentId, bundle, path, seq)
            toast.success(`Restored rev ${seq}`)
            setRestoring(null)
            choose(null)
            await load()
        } catch (e) {
            toast.error("Couldn't restore", e instanceof Error ? e.message : undefined)
        }
    }

    const remove = async () => {
        try {
            await deleteDoc(agentId, bundle, path)
            toast.success(`Deleted ${doc?.title ?? path}`)
            navigate(bundleHref(agentId, bundle))
        } catch (e) {
            setDeleting(false)
            toast.error("Couldn't delete the document", e instanceof Error ? e.message : undefined)
        }
    }

    const from = chatFrom(agentId, params.get('from'))
    const back = from ? { href: from, label: 'the conversation' } : { href: bundleHref(agentId, bundle), label: bundle }
    if (missing) {
        return (
            <PageFrame title={path} meta={`${bundle} / ${path}.md`} back={back}>
                <EmptyState kind="error" title="This document isn’t there" message={`It may have been moved or deleted. ${from ? 'The conversation' : 'Its bundle'} is one step back.`} />
            </PageFrame>
        )
    }

    const at = history.find((r) => r.seq === shown)
    const text = rev !== null ? (old?.seq === rev ? old.content : null) : (doc?.content ?? null)
    const edit = () => navigate(`${docHref(agentId, bundle, path)}?edit=1${from ? `&from=${encodeURIComponent(from)}` : ''}`)

    const links = (label: string, items: DocLink[]) =>
        items.length > 0 && (
            <section className="flex flex-col gap-6">
                <Eyebrow as="h3" count={items.length}>
                    {label}
                </Eyebrow>
                <ul className="flex flex-col gap-2">
                    {items.map((l) => (
                        <li key={l.href}>
                            <Link to={l.href} className="flex items-center gap-6 rounded-xs py-2 type-mono-sm text-fg hover:underline">
                                <Icon name="arrow-right" size={12} tone="muted" />
                                <span className="truncate">{l.label}</span>
                                {l.note && <span className="shrink-0 text-faint">({l.note})</span>}
                            </Link>
                        </li>
                    ))}
                </ul>
            </section>
        )

    return (
        <PageFrame
            fill
            back={back}
            title={doc?.title ?? path}
            meta={`${bundle} / ${path}.md${current ? ` · rev ${current.seq}` : ''}`}
            actions={
                <>
                    <Button variant="ghost" icon="trash" disabled={!doc} onClick={() => setDeleting(true)}>
                        Delete
                    </Button>
                    <Button variant="primary" icon="pencil" disabled={!doc} onClick={edit}>
                        Edit
                    </Button>
                </>
            }
        >
            <Panes
                paneLabel="About this document"
                pane={
                    <>
                        {doc ? (
                            <>
                                {doc.tags.length > 0 && (
                                    <div className="flex flex-wrap gap-6">
                                        {doc.tags.map((t) => (
                                            <Badge key={t}>{t}</Badge>
                                        ))}
                                    </div>
                                )}
                                <KeyValueList
                                    items={[
                                        { label: 'Updated', value: relativeTime(doc.updated_at) },
                                        { label: 'Created', value: absoluteTime(doc.created_at, true) },
                                    ]}
                                />
                                {links(
                                    'Links to',
                                    doc.relations.map((r) => relationLink(agentId, bundle, r))
                                )}
                                {links('Linked from', linkedFrom)}
                            </>
                        ) : (
                            <Skeleton lines={4} />
                        )}
                        <section className="flex flex-col gap-8">
                            <PanelHeader title="History" meta={total ? `${total} revision${total === 1 ? '' : 's'}` : undefined} level={3} />
                            <div className="flex flex-col gap-2">
                                {history.map((r, i) => (
                                    <RevisionRow
                                        key={r.seq}
                                        revision={r.seq}
                                        author={`${authorLabel(r.actor, r.trigger, agentName)}${r.deleted ? ' · deleted' : ''}`}
                                        time={relativeTime(r.created_at)}
                                        selected={shown === r.seq}
                                        onSelect={() => choose(r.seq)}
                                        trailing={i === 0 ? <span className="type-mono-xs text-faint">current</span> : undefined}
                                    />
                                ))}
                                {history.length < total && <LoadMore label="Load older" remaining={total - history.length} onLoad={() => void loadHistory(history.length)} />}
                            </div>
                        </section>
                    </>
                }
            >
                {draft && (
                    <Notice
                        tone="info"
                        title={`Unsaved edit from ${absoluteTime(draft.at)}`}
                        actions={
                            <Button size="sm" variant="secondary" onClick={edit}>
                                Continue editing
                            </Button>
                        }
                    >
                        {draft.stale ? 'The document has changed since you started it.' : 'It is kept in this browser until you save or discard it.'}
                    </Notice>
                )}
                {(rev !== null || view === 'changes') && (
                    <div className="flex flex-wrap items-center gap-12">
                        <span className="flex min-w-0 flex-1 flex-col gap-2">
                            <span className="type-body-strong text-fg">{rev !== null ? `Revision ${rev}` : 'Current version'}</span>
                            {at && <span className="type-mono-xs text-faint">{authorLabel(at.actor, at.trigger, agentName)} · {absoluteTime(at.created_at)}</span>}
                        </span>
                        {rev !== null && (
                            <>
                                <Button variant="ghost" onClick={() => choose(null, 'text')}>
                                    Back to current
                                </Button>
                                <Button variant="primary" onClick={() => setRestoring(rev)}>
                                    Restore rev {rev}
                                </Button>
                            </>
                        )}
                    </div>
                )}
                {shown !== null && (
                    <div className="flex">
                        <FilterTabs
                            aria-label="Show"
                            value={view}
                            onChange={(v) => choose(shown, v)}
                            options={[
                                { value: 'text', label: 'Text' },
                                { value: 'changes', label: 'Changes' },
                            ]}
                        />
                    </div>
                )}
                {view === 'changes' ? (
                    diff ? <DiffView title={`rev ${diff.from_seq} → ${diff.to_seq}`} lines={diffLines(diff)} /> : <Skeleton lines={8} />
                ) : text === null ? (
                    <Skeleton lines={12} />
                ) : (
                    <article className="max-w-[72ch]">
                        <Markdown size="document">{linkWikiLinks(agentId, bundle, text)}</Markdown>
                    </article>
                )}
            </Panes>

            <ConfirmDialog
                open={restoring !== null}
                title={`Restore rev ${restoring}?`}
                message="The document becomes that revision's text as a new revision."
                confirmLabel="Restore"
                onConfirm={() => restoring !== null && void restore(restoring)}
                onCancel={() => setRestoring(null)}
            />
            <ConfirmDialog
                open={deleting}
                title={`Delete ${doc?.title ?? path}?`}
                removes={['The document', 'Links to it from other documents stop resolving']}
                confirmLabel="Delete document"
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(false)}
            />
        </PageFrame>
    )
}

function EditDoc({ agentId, bundle, path }: DocProps) {
    const navigate = useNavigate()
    const mobile = useMobile()
    const key = `memory:${agentId}:${bundle}/${path}`
    const [params] = useSearchParams()
    const from = chatFrom(agentId, params.get('from'))
    const back = from ? docFromChat(agentId, bundle, path, from) : docHref(agentId, bundle, path)
    const [doc, setDoc] = useState<MemoryDoc | null>(null)
    const [text, setText] = useState('')
    const [saving, setSaving] = useState(false)
    const [stale, setStale] = useState<ReturnType<typeof pendingDraft>>(null)

    useEffect(() => {
        getDoc(agentId, bundle, path)
            .then((d) => {
                setDoc(d)
                const pending = pendingDraft(key, d.content)
                setText(pending?.text ?? d.content)
                if (pending?.stale) setStale(pending)
            })
            .catch((e) => toast.error("Couldn't load the document", e.message))
    }, [agentId, bundle, path, key])

    const dirty = doc !== null && text !== doc.content
    const edit = (v: string) => {
        setText(v)
        if (doc) saveDraft(key, v, doc.content)
    }

    const save = async () => {
        if (!doc || !dirty || saving) return
        setSaving(true)
        try {
            await saveDoc(agentId, bundle, path, { title: doc.title, content: text, tags: doc.tags })
            clearDraft(key)
            toast.success('Saved')
            navigate(back)
        } catch (e) {
            toast.error("Couldn't save", e instanceof ApiError || e instanceof Error ? e.message : undefined)
        } finally {
            setSaving(false)
        }
    }

    return (
        <PageFrame
            fill
            back={{ href: back, label: doc?.title ?? path }}
            title={`Edit ${doc?.title ?? path}`}
            meta={`${bundle} / ${path}.md · link other documents with [[bundle/document]]`}
            actions={
                <>
                    {dirty && <UnsavedIndicator>Unsaved · draft kept in this browser</UnsavedIndicator>}
                    <Button variant="ghost" onClick={() => (clearDraft(key), navigate(back))}>
                        {dirty ? 'Discard' : 'Cancel'}
                    </Button>
                    <Button variant="primary" loading={saving} disabled={!dirty} shortcut="Ctrl S" onClick={() => void save()}>
                        Save
                    </Button>
                </>
            }
        >
            <div className="flex min-h-0 flex-1 flex-col gap-16 px-16 py-16 sm:px-24">
                {stale && (
                    <Notice
                        tone="info"
                        title={`Your edit from ${absoluteTime(stale.at)} was restored`}
                        actions={
                            <Button size="sm" variant="ghost" onClick={() => (clearDraft(key), setStale(null), setText(doc?.content ?? ''))}>
                                Start from the current text
                            </Button>
                        }
                    >
                        The document has changed since, so saving it would replace those changes.
                    </Notice>
                )}
                {doc === null ? (
                    <Skeleton lines={14} className="min-h-0 flex-1" />
                ) : (
                    <MarkdownEditor look="compact" mode={mobile ? 'markdown' : undefined} value={text} onChange={edit} onSaveShortcut={() => void save()} className="min-h-[420px] flex-1" />
                )}
            </div>
        </PageFrame>
    )
}
