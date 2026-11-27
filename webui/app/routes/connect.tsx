// `/connect#CODE`, the link the bots send (spec 069). It stands outside the app shell: a
// signed-out person signs in and comes back here, and nothing is connected until they confirm.
import { Button, ConnectConfirm, Wordmark, connectConfirmTitle } from '@vizier/design'
import { useEffect, useState } from 'react'
import { Navigate, useLocation, useNavigate } from 'react-router'
import { transport } from '../api/transport'
import { clearStashedCode, codeFromHash, platformName, stashCode, stashedCode } from '../lib/connect'
import { useConnect } from '../screens/settings/useConnect'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'

const PROFILE = '/settings/profile'

/** The code the address carries, kept in this tab; else the one kept through a sign-in. */
function takeCode(hash: string): string | null {
    const fromLink = codeFromHash(hash)
    if (fromLink) stashCode(fromLink)
    return fromLink ?? stashedCode()
}

export default function Connect() {
    const location = useLocation()
    const navigate = useNavigate()
    const { token, user, load, signOut } = useSession()
    // The desktop app signs its owner in itself: there is no token and no signing out.
    const desktop = transport.authMode === 'desktop'
    const signedIn = desktop || !!token
    // Taken before anything can navigate away, so signing in doesn't lose it.
    const [code, setCode] = useState(() => takeCode(location.hash))

    useEffect(() => {
        if (signedIn && !user) void load()
    }, [signedIn]) // eslint-disable-line react-hooks/exhaustive-deps

    // Once it is kept, the code has no business staying in the address bar.
    useEffect(() => {
        if (!signedIn || !location.hash) return
        setCode(takeCode(location.hash))
        navigate(location.pathname, { replace: true })
    }, [signedIn, location.hash, location.pathname, navigate])

    const leave = () => {
        clearStashedCode()
        navigate(PROFILE, { replace: true })
    }

    const flow = useConnect(user ? code : null, (result) => {
        toast.success(`${platformName(result.link.platform)} account connected`)
        leave()
    })

    if (!signedIn) return <Navigate to={`/login?returnTo=${encodeURIComponent('/connect')}`} replace />

    return (
        <main className="flex min-h-screen items-center justify-center bg-ground px-16 py-32 text-fg">
            <div className="flex w-full max-w-480 flex-col gap-16 rounded-xl border border-border bg-surface p-24">
                <div className="flex justify-center">
                    <Wordmark size={28} />
                </div>
                {code ? (
                    <>
                        <h1 className="text-center type-heading">{connectConfirmTitle(flow)}</h1>
                        <ConnectConfirm
                            {...flow}
                            onCancel={leave}
                            // The code stays, so signing in as someone else comes back to it.
                            onSignOut={desktop ? undefined : signOut}
                        />
                    </>
                ) : (
                    <>
                        <h1 className="text-center type-heading">No code to confirm</h1>
                        <p className="type-ui-para text-muted">
                            Send /connect to a bot on Discord or Telegram, then open its link or type the code on your Profile.
                        </p>
                        <div>
                            <Button onClick={leave}>Go to Profile</Button>
                        </div>
                    </>
                )}
            </div>
        </main>
    )
}
