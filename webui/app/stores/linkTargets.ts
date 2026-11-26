import { create } from 'zustand'
import { linkKey } from '../lib/entityLinks'
import {
    getLinkTargets,
    resolveLinkTargets,
    searchLinkTargets,
    type LinkTargets,
} from '../api/endpoints/linkTargets'

/** One shared empty index: a selector returning a fresh object re-renders on every call. */
export const NO_LINK_TARGETS: LinkTargets = {
    artifacts: [],
    functions: [],
    tasks: [],
    skills: [],
    memory: { bundles: [], docs: [] },
    truncated: {},
}

/** Does the cached index itself settle this target? Only presence counts as an answer. */
export function indexHas(index: LinkTargets, kind: string, name: string): boolean {
    switch (kind) {
        case 'artifact':
            return index.artifacts.includes(name)
        case 'function':
            return index.functions.includes(name)
        case 'task':
            return index.tasks.some((t) => t.slug === name)
        case 'skill':
            return index.skills.some((s) => s.slug === name)
        case 'memory':
            return (
                index.memory.bundles.includes(name) ||
                index.memory.docs.some((d) => `${d.bundle}/${d.path}` === name)
            )
        default:
            return false
    }
}

export type LinkRef = { kind: string; name: string }

export type EntitySuggestion = {
    kind: 'artifact' | 'function' | 'skill' | 'task' | 'memory'
    name: string
    note?: string
}

/** A design menu row that is an entity (not a file, agent or person). */
export function isEntitySuggestion(item: unknown): item is EntitySuggestion {
    const kind = (item as EntitySuggestion | undefined)?.kind
    return kind === 'artifact' || kind === 'function' || kind === 'skill' || kind === 'task' || kind === 'memory'
}

/** How many rows one suggestion pass draws, whatever the agent owns. */
const MAX_SUGGESTIONS = 50
/** The resolve endpoint's own bound; bigger render passes go out in chunks. */
const RESOLVE_CHUNK = 100

/** The index as suggestion rows, narrowed by the token: a kind prefix filters to that kind,
 * the text matches names (or, with no colon typed yet, the kind word itself). */
export function suggestionsFor(index: LinkTargets, kind: string | null, query: string): EntitySuggestion[] {
    const rows: EntitySuggestion[] = [
        ...index.artifacts.map((name) => ({ kind: 'artifact' as const, name, note: 'artifact' })),
        ...index.functions.map((name) => ({ kind: 'function' as const, name, note: 'function' })),
        ...index.tasks.map((t) => ({ kind: 'task' as const, name: t.slug, note: 'task' })),
        ...index.skills.map((s) => ({ kind: 'skill' as const, name: s.slug, note: `skill · ${s.scope}` })),
        ...index.memory.bundles.map((name) => ({ kind: 'memory' as const, name, note: 'memory · bundle' })),
        ...index.memory.docs.map((d) => ({ kind: 'memory' as const, name: `${d.bundle}/${d.path}`, note: 'memory' })),
    ]
    const q = query.toLowerCase()
    const picked = kind
        ? rows.filter((r) => r.kind === kind && r.name.toLowerCase().includes(q))
        : q
          ? rows.filter((r) => r.name.toLowerCase().includes(q) || r.kind.startsWith(q))
          : rows
    return picked.slice(0, MAX_SUGGESTIONS)
}

type LinkTargetsStore = {
    byAgent: Record<string, LinkTargets>
    loaded: Record<string, boolean>
    /** Answers the index could not give, per `kind:name`, so one link resolves once. */
    resolved: Record<string, Record<string, boolean>>
    refresh: (agentId: string) => Promise<void>
    /** Settle every link of a render pass: index first, one batched resolve for the rest. */
    resolve: (agentId: string, links: LinkRef[]) => Promise<Record<string, boolean>>
    /** Server search over one kind — the composer's fallback. */
    search: (agentId: string, kind: string, q: string, limit?: number) => Promise<LinkTargets>
}

export const useLinkTargets = create<LinkTargetsStore>()((set, get) => ({
    byAgent: {},
    loaded: {},
    resolved: {},

    refresh: async (agentId) => {
        try {
            const targets = (await getLinkTargets(agentId)) ?? NO_LINK_TARGETS
            set((cur) => ({
                byAgent: { ...cur.byAgent, [agentId]: targets },
                loaded: { ...cur.loaded, [agentId]: true },
                // The index changed: its old absences mean nothing now.
                resolved: { ...cur.resolved, [agentId]: {} },
            }))
        } catch {
            /* keep the previous index: a failed read is not an empty one */
        }
    },

    resolve: async (agentId, links) => {
        const index = get().byAgent[agentId] ?? NO_LINK_TARGETS
        const held = get().resolved[agentId] ?? {}
        const missing = links.filter((l) => !indexHas(index, l.kind, l.name) && !(linkKey(l.kind, l.name) in held))
        const answers: Record<string, boolean> = { ...held }
        // The endpoint takes 100 links at a time; a long conversation goes out in chunks.
        for (let i = 0; i < missing.length; i += RESOLVE_CHUNK) {
            const chunk = missing.slice(i, i + RESOLVE_CHUNK)
            try {
                const settled = (await resolveLinkTargets(agentId, chunk)) ?? {}
                for (const l of chunk) {
                    const key = linkKey(l.kind, l.name)
                    answers[key] = settled[key] ?? false
                }
                set((cur) => ({ resolved: { ...cur.resolved, [agentId]: answers } }))
            } catch {
                /* an unresolved link renders optimistically live rather than broken */
            }
        }
        const out: Record<string, boolean> = {}
        for (const l of links) {
            const key = linkKey(l.kind, l.name)
            out[key] = indexHas(index, l.kind, l.name) || (answers[key] ?? true)
        }
        return out
    },

    search: async (agentId, kind, q, limit = 20) => {
        try {
            return (await searchLinkTargets(agentId, kind, q, limit)) ?? NO_LINK_TARGETS
        } catch {
            return NO_LINK_TARGETS
        }
    },
}))
