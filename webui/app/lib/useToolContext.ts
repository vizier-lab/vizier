import { useEffect, useState } from 'react'
import { listFunctions } from '../api/endpoints/functions'
import { listIntegrations } from '../api/endpoints/integrations'
import { EMPTY_CONTEXT, type ToolContext } from './toolIcons'

// The agent's functions and MCP server names, for naming the calls on its trail. Read once per
// agent and kept a few minutes: every trail of a conversation shares one read. A part that cannot
// be read (no permission, an older server) is left empty, and the trail guesses from names.

const TTL_MS = 5 * 60_000

type Entry = { at: number; value: ToolContext; pending?: Promise<ToolContext> }
const cache = new Map<string, Entry>()

async function load(agentId: string): Promise<ToolContext> {
    try {
        const [functions, mcp] = await Promise.all([
            Promise.resolve().then(() => listFunctions(agentId)).catch(() => null),
            Promise.resolve().then(() => listIntegrations(agentId, 'mcp')).catch(() => null),
        ])
        const names = new Map<string, string>()
        for (const category of functions?.categories ?? []) {
            for (const tool of category.tools) names.set(tool.call_as || tool.name, category.name)
        }
        return {
            functions: names,
            mcpServers: (mcp?.items ?? []).filter((item) => item.family === 'mcp').map((item) => item.name),
        }
    } catch {
        return EMPTY_CONTEXT
    }
}

/** The cached context, starting a read when there is none or it is stale. */
function read(agentId: string): Entry {
    const now = Date.now()
    let entry = cache.get(agentId)
    if (!entry || (now - entry.at > TTL_MS && !entry.pending)) {
        const pending = load(agentId)
        entry = { at: entry?.at ?? 0, value: entry?.value ?? EMPTY_CONTEXT, pending }
        cache.set(agentId, entry)
        pending.then((value) => cache.set(agentId, { at: Date.now(), value }))
    }
    return entry
}

/** What the trail knows about an agent's own tools; empty until the first read lands. */
export function useToolContext(agentId: string | undefined): ToolContext {
    const [value, setValue] = useState<ToolContext>(() => (agentId ? read(agentId).value : EMPTY_CONTEXT))
    useEffect(() => {
        if (!agentId) {
            setValue(EMPTY_CONTEXT)
            return
        }
        let live = true
        const entry = read(agentId)
        setValue(entry.value)
        entry.pending?.then((next) => live && setValue(next))
        return () => {
            live = false
        }
    }, [agentId])
    return value
}
