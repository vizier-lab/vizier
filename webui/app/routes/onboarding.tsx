import { Button, Notice, TextField } from '@vizier/design'
import { useEffect, useState, type FormEvent } from 'react'
import { Navigate, useNavigate } from 'react-router'
import { setToken } from '../api/client'
import { serverStatus, setupFirstUser } from '../api/endpoints/auth'
import { useSession } from '../stores/session'

const MIN_PASSWORD = 8

/** First run: create the admin account. Home's setup guide takes it from there. */
export default function Onboarding() {
    const navigate = useNavigate()
    const token = useSession((s) => s.token)
    const load = useSession((s) => s.load)
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [confirm, setConfirm] = useState('')
    const [touched, setTouched] = useState(false)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    const [open, setOpen] = useState<boolean | null>(null)

    // Setup is only open while there are no users: afterwards this page has nothing to offer.
    useEffect(() => {
        serverStatus().then((s) => setOpen(s.setup_required), () => setOpen(false))
    }, [])

    const tooShort = password !== '' && password.length < MIN_PASSWORD
    const mismatch = confirm !== '' && confirm !== password
    const ready = username.trim() !== '' && password.length >= MIN_PASSWORD && confirm === password

    const submit = async (e: FormEvent) => {
        e.preventDefault()
        setTouched(true)
        if (!ready) return
        setBusy(true)
        setError(undefined)
        try {
            const { token } = await setupFirstUser(username.trim(), password)
            setToken(token)
            useSession.setState({ token })
            await load()
            navigate('/', { replace: true })
        } catch (err) {
            setError(err instanceof Error ? err.message : 'The account could not be created.')
        } finally {
            setBusy(false)
        }
    }

    if (open === null) return null
    if (!open) return <Navigate to={token ? '/' : '/login'} replace />

    return (
        <form onSubmit={submit} className="flex flex-col gap-16">
            <div className="flex flex-col gap-6">
                <h1 className="type-display text-fg">Welcome to Vizier</h1>
                <p className="type-ui-para text-muted">Create your admin account. You’ll be the superadmin of this Vizier.</p>
            </div>
            {error && <Notice tone="failed" title="Couldn’t create the account">{error}</Notice>}
            <TextField label="Username" size="lg" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" autoFocus />
            <TextField
                label="Password"
                size="lg"
                type="password"
                value={password}
                invalid={tooShort}
                helper={`At least ${MIN_PASSWORD} characters.`}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="new-password"
            />
            <TextField
                label="Confirm Password"
                size="lg"
                type="password"
                value={confirm}
                invalid={mismatch}
                helper={mismatch ? 'Passwords do not match.' : undefined}
                onChange={(e) => setConfirm(e.target.value)}
                autoComplete="new-password"
            />
            <Button type="submit" variant="primary" size="lg" block loading={busy} disabled={touched && !ready}>
                Create account
            </Button>
        </form>
    )
}
