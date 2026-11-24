import { Button, Dialog, EmptyState, FileTree, FilterTabs, ListItem, MemoryGraph, SearchField, Skeleton, SlideOver, TextField, type FileTreeNode } from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Navigate, useNavigate, useParams, useSearchParams } from 'react-router'
import {
    bundleGraph,
    bundlesGraph,
    createDoc,
    deleteBundle,
    exportBundle,
    importBundle,
    listBundles,
    listDocs,
    searchMemory,
    type Bundle,
    type GraphDto,
    type Passage,
} from '../api/endpoints/memory'
import { count, relativeTime } from '../lib/format'
import { bundleStars, neighbourhood, toView } from '../lib/memoryGraph'
import { bundleHref, bundleNode, docHref, docId, type LoadedDocs } from '../lib/memoryTree'
import { useMobile, useBelow } from '../lib/useMobile'
import { MemoryDocument, MemoryStrip } from '../screens/memory/MemoryDocument'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { toast } from '../stores/toasts'

const DOCS_PER_LOAD = 200
const MORE = '\u0000more:'

const slugify = (s: string) =>
    s.toLowerCase().normalize('NFKD').replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')

/**
 * Memory: an explorer of bundles and their documents on the left, the graph on the right. With a
 * bundle open (`/memory/:bundle`) the graph is that bundle's documents; with a document open
 * (`/memory/:bundle/:path`) the document takes the graph's place.
 */
export default function Memory() {
    const { agentId = '', '*': splat = '' } = useParams()
    const [params] = useSearchParams()
    // `/memory/:bundle/:path…`, one splat so the explorer stays mounted between addresses.
    const [rawBundle, ...rawPath] = splat.split('/').filter(Boolean)
    const bundle = rawBundle ? decodeURIComponent(rawBundle) : undefined
    const doc = bundle && rawPath.length ? rawPath.map(decodeURIComponent).join('/').replace(/\.md$/, '') : undefined
    // Links from before documents had their own address.
    const legacyDoc = params.get('doc')
    if (bundle && legacyDoc) return <Navigate replace to={docHref(agentId, bundle, legacyDoc)} />
    return <Explorer key={agentId} agentId={agentId} bundle={bundle} doc={doc} />
}

function Explorer({ agentId, bundle, doc }: { agentId: string; bundle?: string; doc?: string }) {
    const navigate = useNavigate()
    const mobile = useMobile()
    const [bundles, setBundles] = useState<Bundle[] | null>(null)
    const [error, setError] = useState<string>()
    const [docs, setDocs] = useState<Record<string, LoadedDocs>>({})
    const [expanded, setExpanded] = useState<Set<string>>(() => new Set(bundle ? [bundle] : []))
    const [query, setQuery] = useState('')
    const [results, setResults] = useState<Passage[] | null>(null)
    const [creating, setCreating] = useState<'bundle' | 'memory' | null>(null)
    const [deleting, setDeleting] = useState<string | null>(null)
    const [explorerOpen, setExplorerOpen] = useState(false)
    const importer = useRef<HTMLInputElement>(null)

    const loadBundles = useCallback(async () => {
        try {
            setBundles(await listBundles(agentId))
            setError(undefined)
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Memory could not be loaded.')
        }
    }, [agentId])
    useEffect(() => void loadBundles(), [loadBundles])

    const loadDocs = useCallback(
        async (name: string, offset = 0) => {
            try {
                const page = await listDocs(agentId, name, offset, DOCS_PER_LOAD, true)
                setDocs((d) => ({
                    ...d,
                    [name]: { items: offset === 0 ? page.memories : [...(d[name]?.items ?? []), ...page.memories], total: page.total },
                }))
            } catch (e) {
                toast.error(`Couldn't list ${name}`, e instanceof Error ? e.message : undefined)
            }
        },
        [agentId]
    )

    // The open bundle is expanded in the tree, wherever you came from.
    useEffect(() => {
        if (!bundle) return
        setExpanded((s) => (s.has(bundle) ? s : new Set(s).add(bundle)))
    }, [bundle])
    useEffect(() => {
        for (const name of expanded) if (!name.includes('/') && !docs[name] && bundles?.some((b) => b.name === name)) void loadDocs(name)
    }, [expanded, docs, bundles, loadDocs])

    // Passage search, after a pause.
    useEffect(() => {
        const q = query.trim()
        if (q.length < 2) return setResults(null)
        const t = setTimeout(() => searchMemory(agentId, q).then(setResults).catch(() => setResults([])), 300)
        return () => clearTimeout(t)
    }, [agentId, query])

    const nodes = useMemo<FileTreeNode[]>(() => (bundles ?? []).map((b) => bundleNode(b, docs[b.name], `${MORE}${b.name}`)), [bundles, docs])

    const toggle = (id: string, open: boolean) =>
        setExpanded((s) => {
            const n = new Set(s)
            if (open) n.add(id)
            else n.delete(id)
            return n
        })

    const select = (node: FileTreeNode) => {
        if (node.id.startsWith(MORE)) {
            const name = node.id.slice(MORE.length)
            return void loadDocs(name, docs[name]?.items.length ?? 0)
        }
        if (node.kind === 'file') {
            const at = node.id.indexOf('/')
            return navigate(docHref(agentId, node.id.slice(0, at), node.id.slice(at + 1)))
        }
        // A bundle opens its graph; a folder only opens in place.
        if (!node.id.includes('/') && node.id !== bundle) navigate(bundleHref(agentId, node.id), { replace: true })
    }

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
            setDocs(({ [deleting]: _gone, ...rest }) => rest)
            navigate(`/${agentId}/memory`, { replace: true })
            void loadBundles()
        } catch (e) {
            toast.error("Couldn't delete the bundle", e instanceof Error ? e.message : undefined)
        }
    }

    const doImport = async (file: File) => {
        try {
            const r = await importBundle(agentId, file)
            toast.success(`Imported ${r.imported.length} document${r.imported.length === 1 ? '' : 's'}`, r.skipped.length ? `${r.skipped.length} skipped` : undefined)
            setDocs({})
            void loadBundles()
        } catch (e) {
            toast.error("Couldn't import", e instanceof Error ? e.message : undefined)
        }
    }

    const totalDocs = bundles?.reduce((n, b) => n + b.concept_count, 0) ?? 0
    const open = bundles?.find((b) => b.name === bundle)
    const nothing = bundles !== null && bundles.length === 0
    // A narrow window: the explorer becomes a slide-over so the document keeps a readable measure.
    const drawer = useBelow(1280) && !mobile
    // Choosing a document, a bundle, a search hit or a new memory moves the address; the drawer
    // closes with it.
    useEffect(() => setExplorerOpen(false), [bundle, doc])

    const content =
        bundle && doc ? (
            <MemoryDocument agentId={agentId} bundle={bundle} path={doc} />
        ) : mobile ? null : (
            <GraphArea
                agentId={agentId}
                bundles={bundles ?? []}
                open={open}
                onFocus={(name) => navigate(name ? bundleHref(agentId, name) : `/${agentId}/memory`, { replace: true })}
                onOpenDoc={(b, path) => navigate(docHref(agentId, b, path))}
                onExport={(name) => void doExport(name)}
                onDelete={setDeleting}
            />
        )

    const explorer = (
        <>
            <SearchField aria-label="Search memory" placeholder="Search passages…" value={query} onValueChange={setQuery} />
            {results !== null ? (
                results.length === 0 ? (
                    <EmptyState kind="no-results" title="Nothing matched" message="It searches meaning, not just words: try saying it another way." />
                ) : (
                    <div className="-mx-8 flex flex-col gap-2" role="list" aria-label="Passages">
                        {results.map((p, i) => (
                            <ListItem
                                key={`${p.bundle}/${p.path}/${i}`}
                                role="listitem"
                                shape="rounded"
                                title={p.title}
                                subtitle={p.text.length > 140 ? `${p.text.slice(0, 139)}…` : p.text}
                                meta={`${p.bundle} · ${p.score.toFixed(2)}`}
                                href={docHref(agentId, p.bundle, p.path)}
                            />
                        ))}
                    </div>
                )
            ) : error ? (
                <EmptyState kind="error" title="Couldn't load memory" message={error} />
            ) : bundles === null ? (
                <Skeleton lines={6} />
            ) : (
                <FileTree
                    className="-mx-8"
                    label="Bundles and documents"
                    nodes={nodes}
                    expanded={expanded}
                    onToggle={toggle}
                    selectedId={bundle && doc ? docId(bundle, doc) : bundle}
                    onSelect={select}
                    empty={<p className="px-8 type-small text-faint">No bundles yet.</p>}
                />
            )}
        </>
    )

    return (
        <PageFrame
            fill
            title="Memory"
            meta={bundles ? `${count(bundles.length)} bundle${bundles.length === 1 ? '' : 's'} · ${count(totalDocs)} document${totalDocs === 1 ? '' : 's'}` : undefined}
            actions={
                <>
                    <input ref={importer} type="file" accept=".zip" hidden onChange={(e) => e.target.files?.[0] && void doImport(e.target.files[0])} />
                    {drawer && (
                        <Button variant="secondary" icon="layers" onClick={() => setExplorerOpen(true)}>
                            Bundles
                        </Button>
                    )}
                    <Button variant="secondary" icon="upload" onClick={() => importer.current?.click()}>
                        Import
                    </Button>
                    <Button variant="secondary" icon="plus" onClick={() => setCreating('bundle')}>
                        New bundle
                    </Button>
                    <Button variant="primary" icon="plus" onClick={() => setCreating('memory')}>
                        New memory
                    </Button>
                </>
            }
        >
            {nothing ? (
                <div className="grid flex-1 place-items-center p-24">
                    <EmptyState
                        kind="empty"
                        icon="memory"
                        title="Nothing remembered yet"
                        message="Memory is the agent’s library: documents grouped into bundles. The agent writes here as it learns, and so can you."
                        action={
                            <Button variant="primary" icon="plus" onClick={() => setCreating('memory')}>
                                New memory
                            </Button>
                        }
                    />
                </div>
            ) : bundle && doc && mobile ? (
                // A phone has room for one: the document, with its way back to the explorer.
                <MemoryDocument agentId={agentId} bundle={bundle} path={doc} />
            ) : drawer ? (
                // A narrow window keeps two: the document (or graph), and the explorer in a slide-over.
                <>
                    {content}
                    <SlideOver open={explorerOpen} title="Bundles" side="left" onClose={() => setExplorerOpen(false)}>
                        {explorer}
                    </SlideOver>
                </>
            ) : (
                <Panes side="left" paneWidth={320} paneLabel="Explorer" pane={explorer} bare phoneOrder="pane-first">
                    {content}
                </Panes>
            )}

            <NewMemoryDialog
                kind={creating}
                bundle={bundle}
                onClose={() => setCreating(null)}
                onCreate={async (b, title) => {
                    const isBundle = creating === 'bundle'
                    // A bundle exists once it has a document, so a new bundle starts with its index note.
                    const r = await createDoc(agentId, {
                        title: isBundle ? b : title,
                        content: isBundle ? `# ${b}\n\nWhat this bundle holds.\n` : `# ${title}\n\n`,
                        bundle: b,
                        path: isBundle ? 'index' : slugify(title),
                        tags: [],
                    })
                    setCreating(null)
                    setDocs(({ [r.bundle]: _stale, ...rest }) => rest)
                    void loadBundles()
                    // A new memory goes straight to its editor; a new bundle opens in the explorer.
                    navigate(isBundle ? bundleHref(agentId, r.bundle) : `${docHref(agentId, r.bundle, r.path)}?edit=1`)
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

type GraphProps = {
    agentId: string
    bundles: Bundle[]
    open?: Bundle
    onFocus: (bundle: string | null) => void
    onOpenDoc: (bundle: string, path: string) => void
    onExport: (bundle: string) => void
    onDelete: (bundle: string) => void
}

/** The right of the explorer: every bundle as a star, or the open bundle's documents. */
function GraphArea({ agentId, bundles, open, onFocus, onOpenDoc, onExport, onDelete }: GraphProps) {
    const [all, setAll] = useState<GraphDto | null>(null)
    const [one, setOne] = useState<{ bundle: string; graph: GraphDto } | null>(null)
    const [order, setOrder] = useState<'largest' | 'recent' | 'linked'>('largest')
    const [picked, setPicked] = useState<string | null>(null)

    useEffect(() => {
        bundlesGraph(agentId).then(setAll, () => setAll(null))
    }, [agentId, bundles])
    useEffect(() => {
        setPicked(null)
        if (!open) return
        let live = true
        bundleGraph(agentId, open.name).then((g) => live && setOne({ bundle: open.name, graph: g }), () => live && setOne(null))
        return () => {
            live = false
        }
    }, [agentId, open])

    const stars = useMemo(() => {
        if (!all) return null
        const shown = bundleStars(bundles, order)
        const names = new Set(shown.map((b) => b.name))
        const weight = new Map(bundles.map((b) => [b.name, b.concept_count]))
        return {
            nodes: all.nodes.filter((n) => names.has(n.slug)).map((n) => ({ id: n.slug, label: n.title, weight: Math.max(1, weight.get(n.slug) ?? 1) })),
            edges: all.edges.filter((e) => names.has(e.source) && names.has(e.target)).map((e) => ({ from: e.source, to: e.target })),
        }
    }, [all, bundles, order])

    const docsView = useMemo(() => (open && one?.bundle === open.name ? neighbourhood(toView(one.graph), '', 'all', 80) : null), [open, one])

    if (open) {
        return (
            <div className="flex min-h-0 flex-1 flex-col">
                <MemoryStrip
                    back={{ label: 'all bundles', onBack: () => onFocus(null) }}
                    mono
                    title={open.name}
                    meta={`${count(open.concept_count)} document${open.concept_count === 1 ? '' : 's'} · ${count(open.link_count)} link${open.link_count === 1 ? '' : 's'}${open.updated_at ? ` · last written ${relativeTime(open.updated_at)}` : ''}`}
                    actions={
                        <>
                            <Button size="sm" variant="secondary" icon="download" onClick={() => onExport(open.name)}>
                                Export
                            </Button>
                            <Button
                                size="sm"
                                variant="destructive"
                                icon="trash"
                                disabled={open.protected}
                                title={open.protected ? 'The reserved bundle cannot be deleted. Delete its documents individually.' : undefined}
                                onClick={() => onDelete(open.name)}
                            >
                                Delete
                            </Button>
                        </>
                    }
                />
                {!docsView ? (
                    <Skeleton className="m-24 flex-1" />
                ) : docsView.nodes.length === 0 ? (
                    <div className="grid flex-1 place-items-center">
                        <EmptyState kind="empty" icon="memory" title="No documents in this bundle" />
                    </div>
                ) : (
                    <MemoryGraph
                        className="min-h-0 flex-1 rounded-none"
                        nodes={docsView.nodes}
                        edges={docsView.edges}
                        selectedId={picked ?? undefined}
                        onSelect={setPicked}
                        onOpen={(id) => {
                            if (id.startsWith('~')) onFocus(id.slice(1))
                            else onOpenDoc(id.slice(0, id.indexOf('/')), id.slice(id.indexOf('/') + 1))
                        }}
                        caption={`Star size = links · dashed = another bundle · ${docsView.nodes.length} of ${docsView.nodes.length + docsView.hidden} shown · double-click to open`}
                    />
                )}
            </div>
        )
    }

    return (
        <div className="flex min-h-0 flex-1 flex-col">
            <MemoryStrip
                title="All bundles"
                actions={
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
            />
            {!stars ? (
                <Skeleton className="m-24 flex-1" />
            ) : (
                <MemoryGraph
                    className="min-h-0 flex-1 rounded-none"
                    nodes={stars.nodes}
                    edges={stars.edges}
                    selectedId={picked ?? undefined}
                    onSelect={setPicked}
                    onOpen={(id) => onFocus(id)}
                    caption={`Dot size = documents · line = links between bundles · ${stars.nodes.length} of ${bundles.length} shown · double-click to open`}
                />
            )}
        </div>
    )
}

function NewMemoryDialog({ kind, bundle: initial, onClose, onCreate }: { kind: 'bundle' | 'memory' | null; bundle?: string; onClose: () => void; onCreate: (bundle: string, title: string) => Promise<void> }) {
    const [bundle, setBundle] = useState('')
    const [title, setTitle] = useState('')
    const [busy, setBusy] = useState(false)
    useEffect(() => {
        if (kind) (setBundle(kind === 'memory' ? (initial ?? '') : ''), setTitle(''))
    }, [kind, initial])

    const isBundle = kind === 'bundle'
    const b = slugify(bundle)
    const valid = b !== '' && (isBundle || slugify(title) !== '')

    const create = async () => {
        setBusy(true)
        try {
            await onCreate(b, title.trim())
        } catch (e) {
            toast.error("Couldn't create it", e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <Dialog open={kind !== null} title={isBundle ? 'New bundle' : 'New memory'} confirmLabel="" onConfirm={() => {}} onCancel={onClose}>
            <form
                className="flex flex-col gap-16"
                onSubmit={(e) => {
                    e.preventDefault()
                    if (valid) void create()
                }}
            >
                <TextField label="Bundle" mono value={bundle} onChange={(e) => setBundle(e.target.value)} helper={b ? `Saved as ${b}` : 'A short name for a group of documents'} autoFocus={isBundle || !initial} />
                {!isBundle && (
                    <TextField
                        label="Title"
                        value={title}
                        onChange={(e) => setTitle(e.target.value)}
                        helper={slugify(title) ? `${b || 'bundle'}/${slugify(title)}.md · you write it next` : 'You write the document next, in the full editor'}
                        autoFocus={!!initial}
                    />
                )}
                <div className="flex justify-end gap-8">
                    <Button type="button" variant="ghost" onClick={onClose}>
                        Cancel
                    </Button>
                    <Button type="submit" variant="primary" loading={busy} disabled={!valid}>
                        {isBundle ? 'Create bundle' : 'Create and edit'}
                    </Button>
                </div>
            </form>
        </Dialog>
    )
}
