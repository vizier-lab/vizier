import { useEffect, useState } from 'react'
import { Navigate, Outlet } from 'react-router'
import { setupStatus } from '../api/endpoints/auth'
import { transport } from '../api/transport'
import { useSession } from '../stores/session'

/** Signed out → /login; a server with no users yet → /onboarding. */
export default function RequireAuth() {
    const { token, user, loading, load } = useSession()
    const [needsSetup, setNeedsSetup] = useState<boolean | null>(null)

    // The desktop app signs its owner in itself: there is no token and no login page.
    const desktop = transport.authMode === 'desktop'

    useEffect(() => {
        if (desktop) {
            if (!user) void load()
        } else if (!token) {
            setupStatus()
                .then((s) => setNeedsSetup(s.needs_setup))
                .catch(() => setNeedsSetup(false))
        } else if (!user) {
            void load()
        }
    }, [token]) // eslint-disable-line react-hooks/exhaustive-deps

    if (desktop) return loading || !user ? null : <Outlet />
    if (!token) {
        if (needsSetup === null) return null
        return <Navigate to={needsSetup ? '/onboarding' : '/login'} replace />
    }
    if (loading || !user) return null
    return <Outlet />
}
