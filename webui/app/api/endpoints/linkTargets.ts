import { api } from '../client'

export interface TaskTarget {
    slug: string
    title: string
}

export interface SkillTarget {
    slug: string
    scope: 'agent' | 'global'
}

export interface DocTarget {
    bundle: string
    path: string
}

export interface LinkTargets {
    artifacts: string[]
    functions: string[]
    tasks: TaskTarget[]
    skills: SkillTarget[]
    memory: { bundles: string[]; docs: DocTarget[] }
    /** Which kinds a cap cut — absence there is not evidence of absence. */
    truncated: Record<string, boolean>
}

const base = (agentId: string) => `/agents/${agentId}/link-targets`

/** The bounded name index (artifacts latest 200, memory docs latest 500). */
export const getLinkTargets = (agentId: string) => api.get<LinkTargets>(base(agentId))

/** Server-side search over one kind — the composer's fallback when the index came up short. */
export const searchLinkTargets = (agentId: string, kind: string, q: string, limit = 20) =>
    api.get<LinkTargets>(
        `${base(agentId)}?kind=${encodeURIComponent(kind)}&q=${encodeURIComponent(q)}&limit=${limit}`
    )

/** Exact existence per link, keyed `kind:name` — the renderer's broken-state check. */
export const resolveLinkTargets = (agentId: string, links: { kind: string; name: string }[]) =>
    api.post<Record<string, boolean>>(`${base(agentId)}/resolve`, { links })
