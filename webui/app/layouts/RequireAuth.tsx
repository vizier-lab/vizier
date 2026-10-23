import { useEffect, useState } from 'react'
import { Navigate, Outlet } from 'react-router'
import { setupStatus } from '../api/endpoints/auth'
import { useSession } from '../stores/session'

/** Signed out → /login; a server with no users yet → /onboarding. */
export default function RequireAuth() {
    const { token, user, loading, load } = useSession()
    const [needsSetup, setNeedsSetup] = useState<boolean | null>(null)

    useEffect(() => {
        if (!token) {
            setupStatus()
                .then((s) => setNeedsSetup(s.needs_setup))
                .catch(() => setNeedsSetup(false))
        } else if (!user) {
            void load()
        }
    }, [token]) // eslint-disable-line react-hooks/exhaustive-deps

    if (!token) {
        if (needsSetup === null) return null
        return <Navigate to={needsSetup ? '/onboarding' : '/login'} replace />
    }
    if (loading || !user) return null
    return <Outlet />
}
