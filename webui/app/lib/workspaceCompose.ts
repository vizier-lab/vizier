// What the Workspace composer's two menus offer (specs/053-workspace-channel/contracts/http-api.md,
// research R19). `/` at the start lists commands and every member agent's skills; after a chosen
// command, `@` offers only the agents that can take it; otherwise `@` offers agents and people.

import type { MentionMenuItem, MentionMenuMember, SkillMenuItem } from '@vizier/design'
import type { CommandsMenu, WorkspaceMember } from '../api/endpoints/workspaces'
import { mentionQuery } from './mentions'

/** The commands that act on an agent's turn or record, and so can only be aimed at agents. */
const AGENT_ONLY = new Set(['skills', 'abort', 'checkpoint', 'lobotomy'])

/** The slash menu: commands, then skills with the agents that have them. */
export function slashItems(menu: CommandsMenu | null): SkillMenuItem[] {
    if (!menu) return []
    return [
        ...menu.commands.map((c) => ({ name: c.name, description: c.description })),
        ...menu.skills.map((s) => ({ name: s.name, description: s.description, agents: s.agents })),
    ]
}

/** The command word a message starts with (`/greet @…`, `/skill greet @…`), when it is typed in full. */
export function leadingCommand(value: string): string | null {
    const match = /^\s*\/(?:skill\s+)?([A-Za-z0-9_-]+)\s/.exec(value)
    return match ? match[1].toLowerCase() : null
}

export type MentionSuggestion = {
    start: number
    query: string
    items: MentionMenuMember[]
    emptyText: string
}

/** The `@` menu for the token being typed, or none. `me` is left out: nobody mentions themselves. */
export function mentionSuggestion(
    value: string,
    members: readonly WorkspaceMember[],
    menu: CommandsMenu | null,
    me?: string
): MentionSuggestion | null {
    const token = mentionQuery(value)
    if (!token) return null
    const command = leadingCommand(value)
    const skill = command ? menu?.skills.find((s) => s.name.toLowerCase() === command) : undefined
    const agentsOnly = command ? AGENT_ONLY.has(command) || !!skill : false
    const allowedAgents = skill ? new Set(skill.agents) : null
    const q = token.query.toLowerCase()

    const items: MentionMenuMember[] = members
        .filter((m) => m.handle !== me)
        .filter((m) => (agentsOnly ? m.kind === 'agent' : true))
        .filter((m) => (allowedAgents && m.kind === 'agent' ? allowedAgents.has(m.handle) : true))
        .filter((m) => m.kind !== 'agent' || m.available)
        .filter((m) => !q || m.handle.toLowerCase().includes(q) || m.name.toLowerCase().includes(q))
        // Agents first, then people; within each, a name that starts with the query first.
        .sort((a, b) => {
            if (a.kind !== b.kind) return a.kind === 'agent' ? -1 : 1
            const sa = a.handle.toLowerCase().startsWith(q) ? 0 : 1
            const sb = b.handle.toLowerCase().startsWith(q) ? 0 : 1
            return sa - sb || a.handle.localeCompare(b.handle)
        })
        .map((m) => ({ kind: m.kind, id: m.handle, name: m.name }))

    const what = agentsOnly ? (skill ? `an agent with /${command}` : 'an agent') : 'one in this Workspace'
    return { start: token.start, query: token.query, items, emptyText: `Nobody is ${what} called “@${token.query}”.` }
}

export type { MentionMenuItem }
