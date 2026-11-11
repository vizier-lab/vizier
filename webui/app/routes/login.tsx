import { Button, Notice, TextField, motionDurations, motionEases } from '@vizier/design'
import { useEffect, useState, type FormEvent } from 'react'
import { motion } from 'motion/react'
import { useNavigate } from 'react-router'
import { useInsecureNetwork } from '../lib/pwa/origin'
import { useReturnTo } from '../lib/returnTo'
import { useSession } from '../stores/session'
import { InsecureNetworkNotice } from '../screens/shared/InsecureNetworkNotice'

export default function Login() {
    const login = useSession((s) => s.login)
    const navigate = useNavigate()
    const returnTo = useReturnTo('/')
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [error, setError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)
    // Signing in needs the engine: without a connection the sign-in cannot succeed, so the screen
    // says so instead of failing with a message about a password.
    const [offline, setOffline] = useState(() => typeof navigator !== 'undefined' && !navigator.onLine)
    // The browser refuses the worker on this address, so the app cannot be installed here.
    const insecure = useInsecureNetwork()
    useEffect(() => {
        const update = () => setOffline(!navigator.onLine)
        window.addEventListener('online', update)
        window.addEventListener('offline', update)
        return () => {
            window.removeEventListener('online', update)
            window.removeEventListener('offline', update)
        }
    }, [])

    const submit = async (e: FormEvent) => {
        e.preventDefault()
        setBusy(true)
        setError(null)
        try {
            await login(username, password)
            navigate(returnTo, { replace: true })
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Sign-in failed')
        } finally {
            setBusy(false)
        }
    }

    return (
        <motion.form
            onSubmit={submit}
            className="flex flex-col gap-16"
            // The first thing anyone sees: it arrives, it doesn't appear.
            initial={{ opacity: 0, y: 8 }}
            animate={{
                opacity: 1,
                y: 0,
                transition: { duration: motionDurations.slow, ease: motionEases.arrive },
            }}
        >
            <div className="flex flex-col gap-6">
                <h1 className="type-display text-fg">Welcome back</h1>
                <p className="type-ui-para text-muted">Sign in to your digital steward.</p>
            </div>
            {error && <Notice tone="failed" title="Couldn't sign you in">{error}</Notice>}
            {offline && (
                <Notice tone="retrying" title="No connection to Vizier">
                    Signing in needs a connection. It will work as soon as this device is back online.
                </Notice>
            )}
            {insecure && <InsecureNetworkNotice />}
            <TextField label="Username" size="lg" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" autoFocus />
            <TextField label="Password" size="lg" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" placeholder="Enter your password" invalid={error !== null} />
            <Button type="submit" variant="primary" size="lg" block loading={busy}>
                Sign in
            </Button>
            <p className="type-small text-faint">No account? Ask whoever runs this Vizier to invite you.</p>
        </motion.form>
    )
}
