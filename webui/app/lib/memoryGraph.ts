import type { GraphDto, GraphNodeDto } from '../api/endpoints/memory'

export type VNode = { id: string; label: string; weight: number; faded?: boolean }
export type VEdge = { from: string; to: string; dashed?: boolean }

const id = (n: Pick<GraphNodeDto, 'bundle' | 'slug' | 'boundary'>) => (n.boundary ? `~${n.bundle}` : `${n.bundle}/${n.slug}`)

/** Maps a bundle's graph to library nodes: a document's size is how many links touch it. */
export function toView(graph: GraphDto): { nodes: VNode[]; edges: VEdge[] } {
    // Edges name documents by slug; a boundary node stands for another bundle.
    const bySlug = new Map<string, GraphNodeDto>()
    for (const n of graph.nodes) bySlug.set(n.slug, n)
    const degree = new Map<string, number>()
    const edges: VEdge[] = []
    for (const e of graph.edges) {
        const a = bySlug.get(e.source)
        const b = bySlug.get(e.target)
        if (!a || !b || e.broken) continue
        edges.push({ from: id(a), to: id(b), dashed: a.boundary || b.boundary })
        degree.set(id(a), (degree.get(id(a)) ?? 0) + 1)
        degree.set(id(b), (degree.get(id(b)) ?? 0) + 1)
    }
    const nodes = graph.nodes.map((n) => ({ id: id(n), label: n.boundary ? n.bundle : n.title, weight: 1 + (degree.get(id(n)) ?? 0), faded: n.boundary }))
    return { nodes, edges }
}

/** The documents within `hops` links of `root`, nearest first, at most `max` of them. Farther ones are faded. */
export function neighbourhood(view: { nodes: VNode[]; edges: VEdge[] }, root: string, hops: 1 | 2 | 'all', max = 40): { nodes: VNode[]; edges: VEdge[]; hidden: number } {
    if (hops === 'all') {
        const nodes = [...view.nodes].sort((a, b) => b.weight - a.weight).slice(0, max)
        const keep = new Set(nodes.map((n) => n.id))
        return { nodes, edges: view.edges.filter((e) => keep.has(e.from) && keep.has(e.to)), hidden: Math.max(0, view.nodes.length - nodes.length) }
    }
    const dist = new Map<string, number>([[root, 0]])
    let frontier = [root]
    for (let d = 1; d <= hops; d++) {
        const next: string[] = []
        for (const cur of frontier) {
            for (const e of view.edges) {
                const other = e.from === cur ? e.to : e.to === cur ? e.from : null
                if (other && !dist.has(other)) {
                    dist.set(other, d)
                    next.push(other)
                }
            }
        }
        frontier = next
    }
    const ordered = view.nodes.filter((n) => dist.has(n.id)).sort((a, b) => dist.get(a.id)! - dist.get(b.id)! || b.weight - a.weight)
    const nodes = ordered.slice(0, max).map((n) => ({ ...n, faded: n.faded || dist.get(n.id) === 2 }))
    const keep = new Set(nodes.map((n) => n.id))
    return { nodes, edges: view.edges.filter((e) => keep.has(e.from) && keep.has(e.to)), hidden: Math.max(0, ordered.length - nodes.length) }
}

/** The bundles to draw as stars, by one of the three orders the graph offers. */
export function bundleStars(
    bundles: { name: string; concept_count: number; updated_at: string | null; link_count: number }[],
    order: 'largest' | 'recent' | 'linked',
    max = 24
) {
    const key = (b: (typeof bundles)[number]) => (order === 'largest' ? b.concept_count : order === 'linked' ? b.link_count : Date.parse(b.updated_at ?? '') || 0)
    return [...bundles].sort((a, b) => key(b) - key(a)).slice(0, max)
}
