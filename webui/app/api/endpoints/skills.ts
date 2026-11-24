import { api } from '../client'

export interface Skill {
    name: string
    description: string
    keywords: string[]
    version: number
    resources: string[]
    content: string
    agent_id: string | null
    scope: 'agent' | 'global'
    /** sha256 of `content`, to send back so a save can't overwrite a newer one. */
    base_hash: string
    updated?: string
}

export interface SkillInput {
    description?: string
    content?: string
    keywords?: string[]
    base_hash?: string
}

export const skillPath = (scope: 'agent' | 'global', agentId: string) => (scope === 'agent' ? `/agents/${agentId}/skills` : '/skills')

export const getSkill = (scope: 'agent' | 'global', agentId: string, slug: string) =>
    api.get<Skill>(`${skillPath(scope, agentId)}/${encodeURIComponent(slug)}`)

export const createSkill = (scope: 'agent' | 'global', agentId: string, input: { name: string; description: string; content: string; keywords: string[] }) =>
    api.post<Skill>(skillPath(scope, agentId), input)

export const updateSkill = (scope: 'agent' | 'global', agentId: string, slug: string, input: SkillInput) =>
    api.put<Skill>(`${skillPath(scope, agentId)}/${encodeURIComponent(slug)}`, input)

export const deleteSkill = (scope: 'agent' | 'global', agentId: string, slug: string) =>
    api.delete(`${skillPath(scope, agentId)}/${encodeURIComponent(slug)}`)

