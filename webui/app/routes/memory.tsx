import {
    Breadcrumb,
    Button,
    Dialog,
    EmptyState,
    FilterTabs,
    ListItem,
    MemoryGraph,
    Pagination,
    SearchField,
    SelectedBundle,
    Table,
    TableRow,
    TextField,
    Textarea,
} from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import {
    bundleGraph,
    bundlesGraph,
    createDoc,
    deleteBundle,
    exportBundle,
    importBundle,
    listDocs,
    searchMemory,
    type Bundle,
    type GraphDto,
    type MemorySummary,
    type Passage,
} from '../api/endpoints/memory'
import { count, relativeTime } from '../lib/format'
import { bundleStars, neighbourhood, toView } from '../lib/memoryGraph'
import { usePagedList } from '../lib/usePagedList'
import { DocumentPane } from '../screens/memory/DocumentPane'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const DOCS_PER_PAGE = 25

const slugify = (s: string) =>
    s.toLowerCase().normalize('NFKD').replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')

export default function Memory() {
    const { agentId = '', bundle } = useParams()
    return bundle ? <BundleView agentId={agentId} bundle={decodeURIComponent(bundle)} /> : <AllBundles agentId={agentId} />
}

// ---------------------------------------------------------------------------------------------
// All bundles
// ---------------------------------------------------------------------------------------------

function AllBundles({ agentId }: { agentId: string }) {
    const navigate = useNavigate()
    const list = usePagedList<Bundle>(`/agents/${agentId}/memory/bundles`, { sort: 'documents' })
    const [graph, setGraph] = useState<GraphDto | null>(null)
    const [order, setOrder] = useState<'largest' | 'recent' | 'linked'>('largest')
    const [selected, setSelected] = useState<string | null>(null)
    const [query, setQuery] = useState('')
    const [results, setResults] = useState<Passage[] | null>(null)
    const [creating, setCreating] = useState<'bundle' | 'memory' | null>(null)
    const [deleting, setDeleting] = useState<string | null>(null)
    const importer = useRef<HTMLInputElement>(null)

    useEffect(() => {
        bundlesGraph(agentId).then(setGraph).catch(() => {})
    }, [agentId, list.data?.total])

    // Passage search, after a pause.
    useEffect(() => {
        const q = query.trim()
        if (q.length < 2) return setResults(null)
        const t = setTimeout(() => searchMemory(agentId, q).then(setResults).catch(() => setResults([])), 300)
        return () => clearTimeout(t)
    }, [agentId, query])

    const all = list.data?.items ?? []
    const stars = useMemo(() => bundleStars(all, order), [all, order])
    const view = useMemo(() => {
        if (!graph) return null
        const names = new Set(stars.map((b) => b.name))
        const weight = new Map(all.map((b) => [b.name, b.concept_count]))
        return {
            nodes: graph.nodes.filter((n) => names.has(n.slug)).map((n) => ({ id: n.slug, label: n.title, weight: Math.max(1, weight.get(n.slug) ?? 1) })),
            edges: graph.edges.filter((e) => names.has(e.source) && names.has(e.target)).map((e) => ({ from: e.source, to: e.target })),
        }
    }, [graph, stars, all])

    const doExport = async (name: string) => {
        try {
            const blob = await exportBundle(agentId, name)
            const url = URL.createObjectURL(blob)
            const a = document.createElement('a')
            a.href = url
            a.download = `${name}.zip`
            a.click()
            URL.revokeObjectURL(url)
        } catch (e) {
            toast.error("Couldn't export", e instanceof Error ? e.message : undefined)
        }
    }

    const doDelete = async () => {
        if (!deleting) return
        try {
            await deleteBundle(agentId, deleting, true)
            toast.success(`Deleted ${deleting}`)
            setDeleting(null)
            setSelected(null)
            list.reload()
        } catch (e) {
            toast.error("Couldn't delete the bundle", e instanceof Error ? e.message : undefined)
        }
    }

    const doImport = async (file: File) => {
        try {
            const r = await importBundle(agentId, file)
            toast.success(`Imported ${r.imported.length} document${r.imported.length === 1 ? '' : 's'}`, r.skipped.length ? `${r.skipped.length} skipped` : undefined)
            list.reload()
        } catch (e) {
            toast.error("Couldn't import", e instanceof Error ? e.message : undefined)
        }
    }

    const sel = all.find((b) => b.name === selected)

    return (
        <PageFrame
            title="Memory"
            meta={list.data ? `${count(list.data.counts.all ?? 0)} bundle${list.data.counts.all === 1 ? '' : 's'}` : undefined}
            actions={
                <div className="flex flex-wrap items-center gap-8">
                    <SearchField className="w-260" aria-label="Search memory" placeholder="Search passages…" value={query} onValueChange={setQuery} />
                    <input ref={importer} type="file" accept=".zip" hidden onChange={(e) => e.target.files?.[0] && void doImport(e.target.files[0])} />
                    <Button variant="secondary" icon="upload" onClick={() => importer.current?.click()}>
                        Import
                    </Button>
                    <Button variant="secondary" icon="plus" onClick={() => setCreating('bundle')}>
                        New bundle
                    </Button>
                    <Button variant="primary" icon="plus" onClick={() => setCreating('memory')}>
                        New memory
                    </Button>
                </div>
            }
        >
            {results !== null ? (
                results.length === 0 ? (
                    <EmptyState kind="no-results" title="Nothing matched" message="Try different words: it searches meaning, not just text." />
                ) : (
                    <div className="flex flex-col gap-4">
                        {results.map((p, i) => (
                            <ListItem
                                key={`${p.bundle}/${p.path}/${i}`}
                                shape="rounded"
                                title={p.title}
                                subtitle={p.text.length > 160 ? `${p.text.slice(0, 159)}…` : p.text}
                                meta={`${p.bundle} · ${p.score.toFixed(2)}`}
                                href={`/${agentId}/memory/${encodeURIComponent(p.bundle)}?doc=${encodeURIComponent(p.path)}`}
                            />
                        ))}
                    </div>
                )
            ) : (
                <div className="grid grid-cols-1 items-start gap-24 xl:grid-cols-[minmax(0,1fr)_380px]">
                    {view && view.nodes.length > 0 && (
                        <MemoryGraph
                            className="h-[min(680px,calc(100vh-13rem))] min-h-[360px]"
                            nodes={view.nodes}
                            edges={view.edges}
                            selectedId={selected ?? undefined}
                            onSelect={setSelected}
                            onOpen={(id) => navigate(`/${agentId}/memory/${encodeURIComponent(id)}`)}
                            controls={
                                <FilterTabs
                                    aria-label="Order"
                                    value={order}
                                    onChange={setOrder}
                                    options={[
                                        { value: 'largest', label: 'Largest' },
                                        { value: 'recent', label: 'Recently changed' },
                                        { value: 'linked', label: 'Most linked' },
                                    ]}
                                />
                            }
                            caption={`Dot size = documents · line = links between bundles · ${view.nodes.length} of ${list.data?.counts.all ?? view.nodes.length} shown · double-click to open`}
                        />
                    )}
                    <div className="flex min-w-0 flex-col gap-16">
                    <PagedTable<Bundle>
                        label="Bundles"
                        searchLabel="Filter bundles"
                        list={list}
                        rowKey={(b) => b.name}
                        selectedKey={selected ?? undefined}
                        onSelect={(b) => setSelected(b.name)}
                        sorts={[
                            { value: 'documents', label: 'Largest' },
                            { value: 'updated', label: 'Recently changed' },
                            { value: 'links', label: 'Most linked' },
                            { value: 'name', label: 'Name' },
                        ]}
                        columns={[{ label: 'Bundle' }, { label: 'Docs', width: 70, align: 'end' }, { label: 'Updated', width: 90 }]}
                        endAligned={[1]}
                        row={(b) => [
                            <span key="n" className="type-mono-sm text-fg">{b.name}</span>,
                            count(b.concept_count),
                            <span key="u" className="type-mono-xs text-faint">{b.updated_at ? relativeTime(b.updated_at) : '–'}</span>,
                        ]}
                        empty={{
                            icon: 'memory',
                            title: 'Nothing remembered yet',
                            message: 'Memory is the agent’s library: documents grouped into bundles.',
                            action: (
                                <Button variant="primary" icon="plus" onClick={() => setCreating('memory')}>
                                    New memory
                                </Button>
                            ),
                        }}
                    />
                    {sel && (
                        <SelectedBundle
                            name={sel.name}
                            meta={`${count(sel.concept_count)} document${sel.concept_count === 1 ? '' : 's'} · ${count(sel.link_count)} link${sel.link_count === 1 ? '' : 's'}${sel.updated_at ? ` · last written ${relativeTime(sel.updated_at)}` : ''}`}
                            onOpen={() => navigate(`/${agentId}/memory/${encodeURIComponent(sel.name)}`)}
                            onExport={() => void doExport(sel.name)}
                            onDelete={() => setDeleting(sel.name)}
                        />
                    )}
                    </div>
                </div>
            )}

            <NewMemoryDialog
                kind={creating}
                agentId={agentId}
                onClose={() => setCreating(null)}
                onCreated={(b, p) => {
                    setCreating(null)
                    list.reload()
                    navigate(`/${agentId}/memory/${encodeURIComponent(b)}?doc=${encodeURIComponent(p)}`)
                }}
            />

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete ${deleting ?? 'bundle'}?`}
                removes={['The bundle and every document in it', 'Their version history']}
                confirmWord={deleting ?? undefined}
                confirmLabel="Delete bundle"
                onConfirm={() => void doDelete()}
                onCancel={() => setDeleting(null)}
            />
        </PageFrame>
    )
}

function NewMemoryDialog({ kind, agentId, onClose, onCreated }: { kind: 'bundle' | 'memory' | null; agentId: string; onClose: () => void; onCreated: (bundle: string, path: string) => void }) {
    const [bundle, setBundle] = useState('')
    const [title, setTitle] = useState('')
    const [content, setContent] = useState('')
    const [busy, setBusy] = useState(false)
    useEffect(() => {
        if (kind) (setBundle(''), setTitle(''), setContent(''))
    }, [kind])

    const isBundle = kind === 'bundle'
    const b = slugify(bundle)
    const valid = b !== '' && (isBundle || (title.trim() !== '' && content.trim() !== ''))

    const create = async () => {
        setBusy(true)
        try {
            // A bundle exists once it has a document, so a new bundle starts with its index note.
            const r = await createDoc(agentId, {
                title: isBundle ? b : title.trim(),
                content: isBundle ? `# ${b}\n\nWhat this bundle holds.` : content,
                bundle: b,
                path: isBundle ? 'index' : `${slugify(title)}.md`,
                tags: [],
            })
            onCreated(r.bundle, r.path)
        } catch (e) {
            toast.error("Couldn't create it", e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <Dialog open={kind !== null} title={isBundle ? 'New bundle' : 'New memory'} confirmLabel="" onConfirm={() => {}} onCancel={onClose} size="lg">
            <form
                className="flex flex-col gap-16"
                onSubmit={(e) => {
                    e.preventDefault()
                    if (valid) void create()
                }}
            >
                <TextField label="Bundle" mono value={bundle} onChange={(e) => setBundle(e.target.value)} helper={b ? `Saved as ${b}` : 'A short name for a group of documents'} autoFocus />
                {!isBundle && (
                    <>
                        <TextField label="Title" value={title} onChange={(e) => setTitle(e.target.value)} />
                        <Textarea label="Content" rows={6} value={content} onChange={(e) => setContent(e.target.value)} helper="Markdown. Link other documents with [[bundle/document]]." />
                    </>
                )}
                <div className="flex justify-end gap-8">
                    <Button type="button" variant="ghost" onClick={onClose}>Cancel</Button>
                    <Button type="submit" variant="primary" loading={busy} disabled={!valid}>Create</Button>
                </div>
            </form>
        </Dialog>
    )
}

// ---------------------------------------------------------------------------------------------
// Inside a bundle
// ---------------------------------------------------------------------------------------------

function BundleView({ agentId, bundle }: { agentId: string; bundle: string }) {
    const [params, setParams] = useSearchParams()
    const docParam = params.get('doc')
    const [page, setPage] = useState(1)
    const [docs, setDocs] = useState<{ memories: MemorySummary[]; total: number } | null>(null)
    const [graph, setGraph] = useState<GraphDto | null>(null)
    const [hops, setHops] = useState<'1' | '2' | 'all'>('1')
    const [reloads, setReloads] = useState(0)

    const load = useCallback(async () => {
        const [d, g] = await Promise.allSettled([listDocs(agentId, bundle, (page - 1) * DOCS_PER_PAGE, DOCS_PER_PAGE), bundleGraph(agentId, bundle)])
        if (d.status === 'fulfilled') setDocs(d.value)
        if (g.status === 'fulfilled') setGraph(g.value)
    }, [agentId, bundle, page])

    useEffect(() => {
        void load()
    }, [load, reloads])

    const select = (path: string | null) =>
        setParams(
            (p) => {
                const n = new URLSearchParams(p)
                if (path) n.set('doc', path)
                else n.delete('doc')
                return n
            },
            { replace: true }
        )

    const view = useMemo(() => (graph ? toView(graph) : null), [graph])
    const root = docParam ? `${bundle}/${docParam.replace(/\.md$/, '')}` : null
    const shown = useMemo(() => {
        if (!view) return null
        if (!root || !view.nodes.some((n) => n.id === root)) return neighbourhood(view, root ?? '', 'all', 40)
        return neighbourhood(view, root, hops === 'all' ? 'all' : (Number(hops) as 1 | 2), 40)
    }, [view, root, hops])

    const pages = docs ? Math.max(1, Math.ceil(docs.total / DOCS_PER_PAGE)) : 1

    return (
        <PageFrame
            title={bundle}
            meta={docs ? `${count(docs.total)} document${docs.total === 1 ? '' : 's'}` : undefined}
            actions={<Breadcrumb segments={[{ label: 'memory', href: `/${agentId}/memory` }, { label: bundle }]} />}
        >
            {shown && shown.nodes.length > 0 && (
                <MemoryGraph
                            className="h-320"
                    nodes={shown.nodes}
                    edges={shown.edges}
                    selectedId={root ?? undefined}
                    onSelect={(id) => id && id.includes('/') && select(id.slice(id.indexOf('/') + 1))}
                    onOpen={(id) => {
                        if (id.startsWith('~')) location.assign(`/${agentId}/memory/${encodeURIComponent(id.slice(1))}`)
                        else select(id.slice(id.indexOf('/') + 1))
                    }}
                    controls={
                        <FilterTabs
                            aria-label="Reach"
                            value={hops}
                            onChange={setHops}
                            options={[
                                { value: '1', label: '1 hop' },
                                { value: '2', label: '2 hops' },
                                { value: 'all', label: 'Whole bundle' },
                            ]}
                        />
                    }
                    caption={`Star size = links · faded = 2 hops · dashed = another bundle · ${shown.nodes.length} of ${view?.nodes.length ?? shown.nodes.length} shown`}
                />
            )}

            <div className="grid grid-cols-1 gap-24 xl:grid-cols-[1fr_minmax(0,560px)]">
                <section aria-label="Documents" className="flex flex-col gap-12">
                    {docs && docs.total === 0 ? (
                        <EmptyState kind="empty" icon="memory" title="No documents in this bundle" />
                    ) : (
                        <Table
                            label="Documents"
                            columns={[{ label: 'Document' }, { label: 'Tags', width: 160 }, { label: 'Updated', width: 110 }]}
                            footer={<Pagination page={page} pageCount={pages} total={docs?.total ?? 0} perPage={DOCS_PER_PAGE} onPageChange={setPage} compact />}
                        >
                            {(docs?.memories ?? []).map((m) => (
                                <TableRow
                                    key={m.path}
                                    selected={docParam?.replace(/\.md$/, '') === m.path}
                                    onSelect={() => select(m.path)}
                                    cells={[
                                        <span key="t" className="flex min-w-0 flex-col">
                                            <span className="truncate type-body-strong text-fg">{m.title}</span>
                                            <span className="truncate type-mono-xs text-faint">{m.path}.md</span>
                                        </span>,
                                        <span key="g" className="truncate type-mono-xs text-muted">{m.tags.join(', ')}</span>,
                                        <span key="u" className="type-mono-xs text-faint">{relativeTime(m.updated_at)}</span>,
                                    ]}
                                />
                            ))}
                        </Table>
                    )}
                </section>
                {docParam && (
                    <section aria-label="Document" className="min-w-0">
                        <DocumentPane
                            key={docParam}
                            agentId={agentId}
                            bundle={bundle}
                            path={docParam}
                            graph={graph}
                            onClose={() => select(null)}
                            onChanged={() => setReloads((n) => n + 1)}
                        />
                    </section>
                )}
            </div>
        </PageFrame>
    )
}
