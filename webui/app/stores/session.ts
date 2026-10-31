import { create } from 'zustand'
import { setToken, getToken, setUnauthorizedHandler } from '../api/client'
import { transport, transportReady, type DesktopState } from '../api/transport'
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
    paintWindow(theme)
    // The court changes light in one breath: the old and new grounds cross-fade
    // instead of snapping. Browsers without the API, and people who asked for
    // less movement, get the change straight away. So does the desktop window: a view transition
    // snapshots the page through the compositor, which WebKitGTK may not have (WSL, no GPU), and
    // the window crashes on the switch.
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches || transport.authMode === 'desktop'
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

/**
 * The desktop window's frame (its title bar) is native chrome drawn by the OS, so it does not
 * follow `data-theme` on its own: the shell is told which appearance to paint it in. The
 * transport is loaded lazily on the desktop, which is why this waits for it.
 */
function paintWindow(theme: Theme) {
    if (transport.authMode !== 'desktop') return
    void transportReady
        .then(() => {
            const dark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
            return transport.desktop?.theme(dark ? 'dark' : 'light')
        })
        .catch(() => {
            /* a window that cannot be painted keeps its system frame */
        })
}

// With the theme following the system, the frame follows it too as the system switches.
if (typeof window !== 'undefined') {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
        if (readTheme() === 'system') applyTheme('system')
    })
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
