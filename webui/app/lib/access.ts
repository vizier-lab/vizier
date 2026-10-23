import { useSession } from '../stores/session'

export const PERMISSIONS = {
    providers: 'settings:providers',
    users: 'users:manage',
    roles: 'roles:manage',
    createAgent: 'all_agents:create',
    shellConfig: 'agents:shell_config',
} as const

/** Whether the current person may manage integrations on `agent`. */
export function canManageIntegrations(agent: { owner_id?: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    return (
        can('all_agents:integrations:manage') ||
        (can('owned_agents:integrations:manage') && agent.owner_id === user.user_id)
    )
}

export function canViewUsage(): boolean {
    return useSession.getState().user != null
}

export function useCan(permission: string): boolean {
    return useSession((s) => s.user?.permissions.includes(permission) ?? false)
}
