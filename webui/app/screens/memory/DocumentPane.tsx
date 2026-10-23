import { Button, DiffView, DocumentDetail, LoadMore, MarkdownEditor, PanelHeader, RevisionRow, UnsavedIndicator, type DocLink } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { ApiError } from '../../api/client'
import {
    getDoc,
    memoryDiff,
    memoryHistory,
    rollbackDoc,
    saveDoc,
    type GraphDto,
    type MemoryDoc,
    type MemoryRevisionSummary,
} from '../../api/endpoints/memory'
import { absoluteTime, relativeTime } from '../../lib/format'
import { authorLabel, diffLines, type RevisionDiff } from '../../lib/revisions'
import { clearDraft, pendingDraft, saveDraft } from '../../stores/drafts'
import { toast } from '../../stores/toasts'
import { Markdown } from '../shared/Markdown'
import { ConfirmDialog } from '../shared/ConfirmDialog'

const href = (agentId: string, bundle: string, path: string) =>
    `/${agentId}/memory/${encodeURIComponent(bundle)}?doc=${encodeURIComponent(path.replace(/\.md$/, ''))}`

/** Where a relation points: `other.md` is in this bundle, `bundle/slug` in another. */
export function relationLink(agentId: string, bundle: string, relation: string): DocLink {
    const clean = relation.replace(/\.md$/, '')
    const [b, ...rest] = clean.includes('/') ? clean.split('/') : [bundle, clean]
    const path = rest.join('/') || clean
    return { label: clean, href: href(agentId, b, path), note: b !== bundle ? b : undefined }
}

/** `[[bundle/doc]]` and `[[bundle]]` become links into Memory, outside code. */
export function linkWikiLinks(agentId: string, bundle: string, markdown: string): string {
    return markdown
        .split(/(```[\s\S]*?```|`[^`\n]*`)/g)
        .map((part, i) =>
            i % 2 === 1
                ? part
                : part.replace(/\[\[([^\]\n|]+?)(?:\|([^\]\n]+))?\]\]/g, (_m, target: string, label?: string) => {
                      const t = target.trim()
                      const to = t.includes('/') ? relationLink(agentId, bundle, t).href : `/${agentId}/memory/${encodeURIComponent(t)}`
                      return `[${(label ?? t).trim()}](${to})`
                  })
        )
        .join('')
}

type Props = {
    agentId: string
    bundle: string
    path: string
    graph: GraphDto | null
    onClose: () => void
    onChanged: () => void
}

/** One memory document: read it, follow its links, edit it, see and restore its history. */
export function DocumentPane({ agentId, bundle, path, graph, onClose, onChanged }: Props) {
    const key = `memory:${agentId}:${bundle}/${path}`
    const [doc, setDoc] = useState<MemoryDoc | null>(null)
    const [mode, setMode] = useState<'read' | 'edit' | 'history'>('read')
    const [text, setText] = useState('')
    const [saving, setSaving] = useState(false)
    const [history, setHistory] = useState<MemoryRevisionSummary[]>([])
    const [total, setTotal] = useState(0)
    const [selected, setSelected] = useState<number | null>(null)
    const [diff, setDiff] = useState<RevisionDiff | null>(null)
    const [restoring, setRestoring] = useState<number | null>(null)
    const [draft, setDraft] = useState<ReturnType<typeof pendingDraft>>(null)

    const load = useCallback(async () => {
        const d = await getDoc(agentId, bundle, path)
        setDoc(d)
        setText(d.content)
        setDraft(pendingDraft(key, d.content))
    }, [agentId, bundle, path, key])

    useEffect(() => {
        setDoc(null)
        setMode('read')
        setSelected(null)
        setDiff(null)
        load().catch((e) => toast.error("Couldn't load the document", e.message))
    }, [load])

    const loadHistory = useCallback(
        async (offset: number) => {
            const page = await memoryHistory(agentId, bundle, path, offset)
            setTotal(page.total)
            setHistory((h) => (offset === 0 ? page.revisions : [...h, ...page.revisions]))
        },
        [agentId, bundle, path]
    )

    useEffect(() => {
        if (mode === 'history') void loadHistory(0).catch(() => {})
    }, [mode, loadHistory])

    const linkedFrom = useMemo<DocLink[]>(() => {
        if (!graph) return []
        const here = graph.nodes.find((n) => n.slug === path.replace(/\.md$/, '') && n.bundle === bundle && !n.boundary)
        if (!here) return []
        return graph.edges
            .filter((e) => e.target === here.slug && !e.broken)
            .map((e) => graph.nodes.find((n) => n.slug === e.source))
            .filter((n): n is NonNullable<typeof n> => !!n && !n.boundary)
            .map((n) => ({ label: n.title, href: href(agentId, n.bundle, n.slug) }))
    }, [graph, path, bundle, agentId])

    const dirty = doc !== null && text !== doc.content
    const edit = (v: string) => {
        setText(v)
        if (doc) saveDraft(key, v, doc.content)
    }

    const save = async () => {
        if (!doc || !dirty) return
        setSaving(true)
        try {
            await saveDoc(agentId, bundle, path, { title: doc.title, content: text, tags: doc.tags })
            clearDraft(key)
            toast.success('Saved')
            await load()
            setMode('read')
            onChanged()
        } catch (e) {
            toast.error("Couldn't save", e instanceof ApiError || e instanceof Error ? e.message : undefined)
        } finally {
            setSaving(false)
        }
    }

    const open = async (seq: number) => {
        setSelected(seq)
        setDiff(null)
        const prev = history.find((r) => r.seq < seq)?.seq
        try {
            setDiff(await memoryDiff(agentId, bundle, path, seq, prev))
        } catch (e) {
            toast.error("Couldn't load that revision", e instanceof Error ? e.message : undefined)
        }
    }

    const restore = async (seq: number) => {
        try {
            await rollbackDoc(agentId, bundle, path, seq)
            toast.success(`Restored rev ${seq}`)
            setRestoring(null)
            setSelected(null)
            setDiff(null)
            await load()
            await loadHistory(0)
            onChanged()
        } catch (e) {
            toast.error("Couldn't restore", e instanceof Error ? e.message : undefined)
        }
    }

    if (!doc) return <p className="p-24 type-small text-muted">Loading…</p>

    const current = history[0]
    return (
        <div className="flex flex-col gap-16">
            <DocumentDetail
                title={doc.title}
                path={`${bundle} / ${doc.path}.md`}
                tags={doc.tags}
                linksTo={doc.relations.map((r) => relationLink(agentId, bundle, r))}
                linkedFrom={linkedFrom}
                provenance={`Updated ${absoluteTime(doc.updated_at)} · created ${absoluteTime(doc.created_at)}`}
                onEdit={mode === 'edit' ? undefined : () => setMode('edit')}
                onHistory={() => setMode((m) => (m === 'history' ? 'read' : 'history'))}
                onClose={onClose}
            />
            {mode === 'read' && <Markdown size="document">{linkWikiLinks(agentId, bundle, doc.content)}</Markdown>}
            {mode === 'edit' && (
                <div className="flex flex-col gap-12">
                    {draft && !dirty && (
                        <div className="flex items-center gap-12 rounded-lg border border-border-strong bg-surface px-16 py-12 type-small text-muted">
                            <span className="flex-1">You have an unsaved draft from {absoluteTime(draft.at)}.</span>
                            <Button size="sm" variant="secondary" onClick={() => (setText(draft.text), setDraft(null))}>
                                Restore draft
                            </Button>
                        </div>
                    )}
                    <MarkdownEditor look="compact" value={text} onChange={edit} onSaveShortcut={() => void save()} className="h-480" />
                    <div className="flex items-center justify-end gap-12">
                        {dirty && <UnsavedIndicator>Unsaved · draft kept in this browser</UnsavedIndicator>}
                        <Button variant="ghost" onClick={() => (clearDraft(key), setText(doc.content), setMode('read'))}>
                            Cancel
                        </Button>
                        <Button variant="primary" loading={saving} disabled={!dirty} onClick={() => void save()}>
                            Save
                        </Button>
                    </div>
                </div>
            )}
            {mode === 'history' && (
                <section aria-label="History" className="flex flex-col gap-8">
                    <PanelHeader title="History" meta={total ? `${total} revision${total === 1 ? '' : 's'}` : undefined} level={3} />
                    {history.map((r) => (
                        <RevisionRow
                            key={r.seq}
                            revision={r.seq}
                            author={`${authorLabel(r.actor, r.trigger)}${r.deleted ? ' · deleted' : ''}`}
                            time={relativeTime(r.created_at)}
                            selected={selected === r.seq}
                            onSelect={() => void open(r.seq)}
                        />
                    ))}
                    {history.length < total && <LoadMore label="Load older" remaining={total - history.length} onLoad={() => void loadHistory(history.length)} />}
                    {selected !== null && diff && (
                        <DiffView
                            title={`rev ${diff.from_seq} → ${diff.to_seq}`}
                            lines={diffLines(diff)}
                            restoreLabel={selected !== current?.seq ? `Restore rev ${selected}` : undefined}
                            onRestore={selected !== current?.seq ? () => setRestoring(selected) : undefined}
                        />
                    )}
                </section>
            )}
            <ConfirmDialog
                open={restoring !== null}
                title={`Restore rev ${restoring}?`}
                message="The document becomes that revision's text as a new revision."
                confirmLabel="Restore"
                onConfirm={() => restoring !== null && void restore(restoring)}
                onCancel={() => setRestoring(null)}
            />
        </div>
    )
}
