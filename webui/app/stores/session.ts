import { create } from 'zustand'
import { setToken, getToken, setUnauthorizedHandler, redirectToLogin } from '../api/client'
import { transport, transportReady, type DesktopState } from '../api/transport'
import { currentUser, login as apiLogin, type CurrentUser } from '../api/endpoints/auth'
import type { Agent } from '../api/endpoints/agents'

type SessionStore = {
    token: string | null
    user: CurrentUser | null
    loading: boolean
    login: (username: string, password: string) => Promise<void>
    load: () => Promise<void>
    signOut: () => void
    can: (permission: string) => boolean
    canEditAgent: (agent: Pick<Agent, 'owner_id'>) => boolean
}

export const useSession = create<SessionStore>()((set, get) => ({
    token: typeof window === 'undefined' ? null : getToken(),
    user: null,
    loading: true,

    login: async (username, password) => {
        const { token } = await apiLogin(username, password)
        setToken(token)
        set({ token })
        await get().load()
    },

    load: async () => {
        await transportReady
        if (transport.authMode === 'desktop') {
            // The shell signs the bound owner in; wait for the engine, then read who that is.
            const shell = transport.desktop!
            const ready = (s: DesktopState) => s.state === 'ready' || s.state === 'degraded'
            if (!ready(await shell.state())) {
                await new Promise<void>((resolve) => {
                    void shell.onState((s) => ready(s) && resolve())
                })
            }
            try {
                set({ user: await currentUser(), token: null, loading: false })
            } catch {
                set({ user: null, loading: false })
            }
            return
        }
        if (!getToken()) {
            set({ user: null, token: null, loading: false })
            return
        }
        try {
            set({ user: await currentUser(), token: getToken(), loading: false })
        } catch {
            set({ user: null, loading: false })
        }
    },

    signOut: () => {
        if (transport.authMode === 'desktop') return
        setToken(null)
        set({ token: null, user: null })
    },

    can: (permission) => !!get().user?.permissions.includes(permission),

    canEditAgent: (agent) => {
        const { user, can } = get()
        if (!user) return false
        // Mirrors the server's `user_can_edit_agent`: an owner may always edit their own agent.
        return can('all_agents:edit') || agent.owner_id === user.user_id
    },
}))

setUnauthorizedHandler(() => {
    useSession.getState().signOut()
    if (typeof location !== 'undefined') redirectToLogin()
})
