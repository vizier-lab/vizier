/** How the permissions a role can hold read to a person, grouped as the Roles page shows them. */
export const PERMISSION_GROUPS: { title: string; items: { permission: string; label: string }[] }[] = [
    {
        title: 'Agents',
        items: [
            { permission: 'all_agents:view', label: 'See every agent' },
            { permission: 'owned_agents:view', label: 'See agents they own or that are shared with them' },
            { permission: 'all_agents:create', label: 'Create agents' },
            { permission: 'all_agents:edit', label: 'Edit any agent' },
            { permission: 'owned_agents:edit', label: 'Edit agents they own' },
            { permission: 'all_agents:delete', label: 'Delete any agent' },
            { permission: 'owned_agents:delete', label: 'Delete agents they own' },
            { permission: 'agents:shell_config', label: 'Set up an agent’s shell' },
        ],
    },
    {
        title: 'Integrations',
        items: [
            { permission: 'all_agents:integrations:manage', label: 'Manage every agent’s integrations' },
            { permission: 'owned_agents:integrations:manage', label: 'Manage integrations of agents they own' },
        ],
    },
    {
        title: 'Chats',
        items: [
            { permission: 'all_agents:show_discord_chats', label: 'See every agent’s Discord chats' },
            { permission: 'owned_agents:show_discord_chats', label: 'See Discord chats of agents they own' },
            { permission: 'all_agents:show_telegram_chats', label: 'See every agent’s Telegram chats' },
            { permission: 'owned_agents:show_telegram_chats', label: 'See Telegram chats of agents they own' },
        ],
    },
    {
        title: 'Account',
        items: [
            { permission: 'settings:password', label: 'Change their own password' },
            { permission: 'settings:api_keys', label: 'Create API keys' },
        ],
    },
    {
        title: 'Administration',
        items: [
            { permission: 'users:manage', label: 'Manage users' },
            { permission: 'roles:manage', label: 'Manage roles' },
            { permission: 'settings:providers', label: 'Manage providers' },
        ],
    },
]

/** The group a permission not in the table above lands in, with its name as its label. */
export function groupsFor(available: string[]) {
    const known = new Set(PERMISSION_GROUPS.flatMap((g) => g.items.map((i) => i.permission)))
    const extra = available.filter((p) => !known.has(p))
    const groups = PERMISSION_GROUPS.map((g) => ({ ...g, items: g.items.filter((i) => available.includes(i.permission)) })).filter((g) => g.items.length)
    return extra.length ? [...groups, { title: 'Other', items: extra.map((permission) => ({ permission, label: permission })) }] : groups
}

/** "all permissions", "4 permissions", "1 permission". */
export function permissionCount(n: number, total: number): string {
    return n >= total && total > 0 ? 'all permissions' : `${n} permission${n === 1 ? '' : 's'}`
}
