import type { IntegrationEvent, IntegrationItem, IntegrationState } from '../api/endpoints/integrations'

/** Built-in tool categories and addon ids: an MCP server named like one gets `_mcp` after it. */
const TAKEN_CATEGORIES = new Set([
    'memory', 'identity', 'scheduling', 'agents', 'skills', 'files', 'scratchpad', 'web', 'audio', 'media',
    'discord', 'telegram', 'webui', 'shell', 'integrations', 'brave_search',
])

/** The category an MCP server's tools are listed under. */
export function mcpCategory(name: string): string {
    return TAKEN_CATEGORIES.has(name) ? `${name}_mcp` : name
}

export type CardState = 'reading' | 'failed' | 'installed' | 'disabled'
export type ServerState = 'connected' | 'reconnecting' | 'disabled' | 'failed'

/** A channel or addon card's status. */
export function cardState(s: IntegrationState, enabled: boolean): CardState {
    if (!enabled || s.state === 'disabled') return 'disabled'
    if (s.state === 'failed' || s.state === 'unreachable' || s.state === 'incomplete') return 'failed'
    if (s.state === 'running') return 'reading'
    // Still connecting, or waiting to try again.
    return 'installed'
}

/** An MCP server row's status. */
export function serverState(s: IntegrationState, enabled: boolean): ServerState {
    if (!enabled || s.state === 'disabled') return 'disabled'
    switch (s.state) {
        case 'running':
            return 'connected'
        case 'failed':
        case 'unreachable':
        case 'incomplete':
            return 'failed'
        default:
            return 'reconnecting'
    }
}

/** The line under a status: why it is down, and when it tries again. */
export function stateDetail(s: IntegrationState, now: number = Date.now()): string | undefined {
    if (s.state === 'incomplete') return `needs ${s.missing.join(', ')}`
    if (s.state === 'reconnecting') {
        const left = s.next_retry_at ? Math.max(0, Math.round((new Date(s.next_retry_at).getTime() - now) / 1000)) : null
        return left == null ? s.last_error : `${s.last_error} · retry in ${left}s`
    }
    if (s.state === 'failed' || s.state === 'unreachable') return s.last_error
    return undefined
}

export const isDown = (item: IntegrationItem) => ['reconnecting', 'failed', 'unreachable'].includes(item.status.state)

const ACTION_WORDS: Record<IntegrationEvent['action'], string> = {
    created: 'added',
    edited: 'changed',
    enabled: 'switched on',
    disabled: 'switched off',
    deleted: 'deleted',
    unlinked: 'unlinked',
    grant_on: 'let the agent manage its own',
    grant_off: 'stopped the agent managing its own',
}

/** One log line: "added MCP server filesystem", "changed settings token, args of …". */
export function eventText(e: IntegrationEvent): string {
    const what = e.action === 'grant_on' || e.action === 'grant_off' ? `${e.family} integrations` : e.integration_name
    const fields = e.action === 'edited' && e.fields.length ? ` (${e.fields.join(', ')})` : ''
    return `${ACTION_WORDS[e.action]} ${what}${fields}`
}

/** The tools an installed addon adds, until the API reports them itself. */
export const ADDON_TOOLS: Record<string, { name: string; description: string }[]> = {
    brave_search: [
        { name: 'brave_search_web', description: 'Search the web.' },
        { name: 'brave_search_news', description: 'Search recent news.' },
    ],
}
