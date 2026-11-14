import type { RevisionDiff } from '../../lib/revisions'
import { api } from '../client'

// An agent's functions: the tools it saved from its own scripts (spec 061). The server calls them
// custom tools; people see them as functions. People inspect, edit and govern them; only the
// agent creates them, so there is no create call.

/** Who made a version or a change. */
export type FunctionAuthor = { kind: 'agent' } | { kind: 'dream'; dream_cycle_id: string } | { kind: 'person'; name: string }

export type FunctionSummary = {
    id: string
    name: string
    call_as: string
    category: string
    description: string
    version: number
    author: FunctionAuthor
    updated_at: number
    last_called_at: number | null
    enabled: boolean
    disabled_by: FunctionAuthor | null
    needs_attention: { missing: string[] } | null
    model_backed: boolean
}

export type FunctionCategory = {
    name: string
    description: string
    updated_by: FunctionAuthor | null
    tools: FunctionSummary[]
}

export type FunctionList = { categories: FunctionCategory[]; tool_names_known: boolean }

export type FunctionDetail = {
    id: string
    name: string
    call_as: string
    category: string
    enabled: boolean
    disabled_by: FunctionAuthor | null
    current_version: number
    version: number
    author: FunctionAuthor
    created_at?: number
    description: string
    input_schema: unknown
    output_description: string | null
    script: string
    calls: string[]
    callers: string[]
    model_backed: boolean
    needs_attention: { missing: string[] } | null
    updated_at: number
    last_called_at: number | null
}

export type FunctionVersion = {
    version: number
    author: FunctionAuthor
    trigger: 'create' | 'update' | 'rollback'
    at: number
    name: string
    category: string
    reason?: string
    restored_from?: number
}

export type FunctionDiff = {
    from: number
    to: number
    description: RevisionDiff
    input_schema: RevisionDiff
    script: RevisionDiff
    category?: { from: string; to: string }
    renamed?: { from: string; to: string }
}

/** One problem with a saved definition, as the agent's own tools report it. */
export type FunctionProblem = { code: string; message: string; [key: string]: unknown }

export type FunctionEdit = {
    expected_version: number
    name?: string
    category?: string
    category_description?: string
    description?: string
    input_schema?: unknown
    output_description?: string
    script?: string
    reason?: string
}

const base = (agentId: string) => `/agents/${encodeURIComponent(agentId)}/custom-tools`
const tool = (agentId: string, id: string) => `${base(agentId)}/${encodeURIComponent(id)}`

export const listFunctions = (agentId: string) => api.get<FunctionList>(base(agentId))

export const getFunction = (agentId: string, id: string, version?: number) =>
    api.get<FunctionDetail>(`${tool(agentId, id)}${version ? `?version=${version}` : ''}`)

export const editFunction = (agentId: string, id: string, edit: FunctionEdit) =>
    api.put<{ version: number; tool: { name: string } }>(tool(agentId, id), edit)

export const deleteFunction = (agentId: string, id: string) => api.delete<void>(tool(agentId, id))

export const setFunctionEnabled = (agentId: string, id: string, enabled: boolean) =>
    api.put<{ enabled: boolean }>(`${tool(agentId, id)}/enabled`, { enabled })

export const listFunctionVersions = (agentId: string, id: string) =>
    api.get<{ versions: FunctionVersion[]; next: number | null }>(`${tool(agentId, id)}/versions?limit=200`)

export const diffFunction = (agentId: string, id: string, from: number, to?: number) =>
    api.get<FunctionDiff>(`${tool(agentId, id)}/diff?from=${from}${to ? `&to=${to}` : ''}`)

export const rollbackFunction = (agentId: string, id: string, toVersion: number, expectedVersion: number) =>
    api.post<{ version: number }>(`${tool(agentId, id)}/rollback`, { to_version: toVersion, expected_version: expectedVersion })

export const updateFunctionCategory = (agentId: string, name: string, change: { description?: string; new_name?: string }) =>
    api.put<unknown>(`/agents/${encodeURIComponent(agentId)}/custom-tool-categories/${encodeURIComponent(name)}`, change)
