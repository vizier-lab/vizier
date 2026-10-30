import { create } from 'zustand'
import { setToken, getToken, setUnauthorizedHandler } from '../api/client'
import { currentUser, login as apiLogin, type CurrentUser } from '../api/endpoints/auth'
import type { Agent } from '../api/endpoints/agents'

export type Theme = 'system' | 'dark' | 'light'
const THEME_KEY = 'vizier.theme'

function readTheme(): Theme {
    try {
        const t = localStorage.getItem(THEME_KEY)
        return t === 'dark' || t === 'light' ? t : 'system'
    } catch {
        return 'system'
    }
}

export function applyTheme(theme: Theme) {
    if (typeof document === 'undefined') return
    const el = document.documentElement
    const apply = () => {
        if (theme === 'system') el.removeAttribute('data-theme')
        else el.setAttribute('data-theme', theme)
    }
    // The court changes light in one breath: the old and new grounds cross-fade
    // instead of snapping. Browsers without the API, and people who asked for
    // less movement, get the change straight away.
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const start = (
        document as Document & {
            startViewTransition?: (cb: () => void) => unknown
        }
    ).startViewTransition
    if (reduced || !start) {
        apply()
        return
    }
    start.call(document, apply)
}

type SessionStore = {
    token: string | null
    user: CurrentUser | null
    loading: boolean
    theme: Theme
    login: (username: string, password: string) => Promise<void>
    load: () => Promise<void>
    signOut: () => void
    setTheme: (theme: Theme) => void
    /** The other of dark and light than the one showing now, whatever chose it. */
    toggleTheme: () => void
    can: (permission: string) => boolean
    canEditAgent: (agent: Pick<Agent, 'owner_id'>) => boolean
}

export const useSession = create<SessionStore>()((set, get) => ({
    token: typeof window === 'undefined' ? null : getToken(),
    user: null,
    loading: true,
    theme: typeof window === 'undefined' ? 'system' : readTheme(),

    login: async (username, password) => {
        const { token } = await apiLogin(username, password)
        setToken(token)
        set({ token })
        await get().load()
    },

    load: async () => {
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
        setToken(null)
        set({ token: null, user: null })
    },

    setTheme: (theme) => {
        try {
            if (theme === 'system') localStorage.removeItem(THEME_KEY)
            else localStorage.setItem(THEME_KEY, theme)
        } catch {
            /* the choice lasts until reload */
        }
        applyTheme(theme)
        set({ theme })
    },

    toggleTheme: () => {
        const { theme, setTheme } = get()
        const showing = theme === 'system' ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : theme
        setTheme(showing === 'dark' ? 'light' : 'dark')
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
    if (typeof location !== 'undefined' && location.pathname !== '/login') location.assign('/login')
})
