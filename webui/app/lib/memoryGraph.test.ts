import { expect, it } from 'vitest'
import { bundleStars, neighbourhood, toView } from './memoryGraph'

const graph = {
    nodes: [
        { slug: 'a', bundle: 'infra', title: 'A', tags: [], boundary: false },
        { slug: 'b', bundle: 'infra', title: 'B', tags: [], boundary: false },
        { slug: 'c', bundle: 'infra', title: 'C', tags: [], boundary: false },
        { slug: 'ops', bundle: 'ops', title: 'ops', tags: [], boundary: true },
    ],
    edges: [
        { source: 'a', target: 'b', broken: false },
        { source: 'b', target: 'c', broken: false },
        { source: 'a', target: 'ops', broken: false },
        { source: 'a', target: 'ghost', broken: true },
    ],
}

it('maps links to sizes, and other bundles to dashed edges', () => {
    const v = toView(graph)
    expect(v.nodes.find((n) => n.id === 'infra/a')?.weight).toBe(3)
    expect(v.edges).toContainEqual({ from: 'infra/a', to: '~ops', dashed: true })
    expect(v.edges.some((e) => e.to.includes('ghost'))).toBe(false)
})

it('keeps one hop, then fades the second', () => {
    const v = toView(graph)
    const one = neighbourhood(v, 'infra/a', 1)
    expect(one.nodes.map((n) => n.id).sort()).toEqual(['infra/a', 'infra/b', '~ops'])
    const two = neighbourhood(v, 'infra/a', 2)
    expect(two.nodes.find((n) => n.id === 'infra/c')?.faded).toBe(true)
    expect(two.nodes.find((n) => n.id === 'infra/b')?.faded).toBeFalsy()
})

it('caps what is drawn and says how many are hidden', () => {
    const many = { nodes: Array.from({ length: 60 }, (_, i) => ({ id: `n${i}`, label: `${i}`, weight: i })), edges: [] }
    const r = neighbourhood(many, 'n0', 'all', 40)
    expect(r.nodes).toHaveLength(40)
    expect(r.hidden).toBe(20)
    expect(r.nodes[0].id).toBe('n59')
})

it('orders bundles three ways', () => {
    const b = [
        { name: 'x', concept_count: 5, updated_at: '2026-10-01T00:00:00Z', link_count: 1 },
        { name: 'y', concept_count: 2, updated_at: '2026-10-20T00:00:00Z', link_count: 9 },
    ]
    expect(bundleStars(b, 'largest')[0].name).toBe('x')
    expect(bundleStars(b, 'recent')[0].name).toBe('y')
    expect(bundleStars(b, 'linked')[0].name).toBe('y')
})
