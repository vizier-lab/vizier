import { Button, Notice, TextField } from '@vizier/design'
import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router'
import { useSession } from '../stores/session'

export default function Login() {
    const login = useSession((s) => s.login)
    const navigate = useNavigate()
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [error, setError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)

    const submit = async (e: FormEvent) => {
        e.preventDefault()
        setBusy(true)
        setError(null)
        try {
            await login(username, password)
            navigate('/', { replace: true })
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Sign-in failed')
        } finally {
            setBusy(false)
        }
    }

    return (
        <form onSubmit={submit} className="flex flex-col gap-16">
            <div className="flex flex-col gap-6">
                <h1 className="type-display text-fg">Welcome back</h1>
                <p className="type-ui-para text-muted">Sign in to your digital steward.</p>
            </div>
            {error && <Notice tone="failed" title="Couldn't sign you in">{error}</Notice>}
            <TextField label="Username" size="lg" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" autoFocus />
            <TextField label="Password" size="lg" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" placeholder="Enter your password" invalid={error !== null} />
            <Button type="submit" variant="primary" size="lg" block loading={busy}>
                Sign in
            </Button>
            <p className="type-small text-faint">No account? Ask whoever runs this Vizier to invite you.</p>
        </form>
    )
}
