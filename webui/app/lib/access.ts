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

/** Whether the current person may manage the secrets of `agent`. Mirrors the server rule. */
export function canManageSecrets(agent: { owner_id?: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    return (
        can('all_agents:secret') ||
        (can('owned_agents:secret') && agent.owner_id === user.user_id)
    )
}

/** Whether the current person may see `agent`'s artifact gallery and content. Mirrors
 * `src/artifacts/access.rs`: manage implies view, owned requires ownership. */
export function canViewArtifacts(agent: { owner_id?: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    return (
        can('all_agents:view_artifacts') ||
        can('all_agents:manage_artifacts') ||
        ((can('owned_agents:view_artifacts') || can('owned_agents:manage_artifacts')) &&
            agent.owner_id === user.user_id)
    )
}

/** Whether the current person may promote or delete `agent`'s artifacts. */
export function canManageArtifacts(agent: { owner_id?: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    return (
        can('all_agents:manage_artifacts') ||
        (can('owned_agents:manage_artifacts') && agent.owner_id === user.user_id)
    )
}

/** Whether the current person may see and use `agent`'s projects. Mirrors
 * `crates/vizier-core/src/projects/access.rs`: view + one project permission. The server is
 * still the authority for a direct URL. */
export function canUseProjects(agent: { owner_id?: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    // A listed agent is one the person can already view; the project permission decides the rest.
    return (
        can('all_agents:projects') ||
        (can('owned_agents:projects') && agent.owner_id === user.user_id)
    )
}

/** Whether the current person may rename, remove or toggle `project`: its owner or the
 * whole-workspace permission. */
export function canManageProject(project: { owner_id: string }): boolean {
    const { user, can } = useSession.getState()
    if (!user) return false
    return project.owner_id === user.user_id || can('all_agents:projects')
}

export function canViewUsage(): boolean {
    return useSession.getState().user != null
}

export function useCan(permission: string): boolean {
    return useSession((s) => s.user?.permissions.includes(permission) ?? false)
}
