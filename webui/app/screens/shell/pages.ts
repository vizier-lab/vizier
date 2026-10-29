import type { EssentialPage, MenuPage } from '@vizier/design'

export type AgentPage = EssentialPage | MenuPage | 'chat'

/** Where each of an agent's pages lives, under `/:agentId`. */
export const PAGE_PATH: Record<AgentPage, string> = {
    chat: '/chat',
    memory: '/memory',
    artifacts: '/artifacts',
    tasks: '/tasks',
    skills: '/skills',
    dreams: '/dream',
    core: '/core',
    channels: '/integrations/channels',
    addons: '/integrations/addons',
    mcp: '/integrations/mcp',
    usage: '/usage',
    secrets: '/secrets',
    config: '/config',
}

/** Which of an agent's pages the path is on, if any. */
export function pageOf(pathname: string): AgentPage | undefined {
    const rest = '/' + pathname.split('/').slice(2).join('/')
    return (Object.entries(PAGE_PATH) as [AgentPage, string][]).find(([, p]) => rest === p || rest.startsWith(p + '/'))?.[0]
}

/** The shell has three shapes: no column (Home, New agent), the agent's column, or Settings'. */
export type ShellView = 'bare' | 'agent' | 'settings'

export function viewOf(pathname: string, routeAgent: string | undefined): ShellView {
    if (pathname.startsWith('/settings')) return 'settings'
    return routeAgent ? 'agent' : 'bare'
}
