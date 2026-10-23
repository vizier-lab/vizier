import { Button, Callout, Notice, ProviderTile, Stepper, TextField } from '@vizier/design'
import { useEffect, useState, type FormEvent } from 'react'
import { Navigate, useNavigate, useSearchParams } from 'react-router'
import { setToken } from '../api/client'
import { createAgent } from '../api/endpoints/agents'
import { serverStatus, setupFirstUser } from '../api/endpoints/auth'
import { allProviders, type ProviderSummary } from '../api/endpoints/catalogue'
import { connectedProviders } from '../api/endpoints/providers'
import { emptyForm, slugOf, toInput } from '../lib/agentForm'
import { ModelSelect } from '../screens/agents/ModelSelect'
import { ConnectForm } from '../screens/providers/ConnectForm'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'

type Step = 'account' | 'provider' | 'agent'
const MIN_PASSWORD = 8

/** First run: create the admin account, connect a provider, make a first agent, then chat. */
export default function Onboarding() {
    const [params, setParams] = useSearchParams()
    const token = useSession((s) => s.token)
    const [open, setOpen] = useState<boolean | null>(null)

    // Setup is only open while there are no users: afterwards this page has nothing to offer.
    useEffect(() => {
        serverStatus().then((s) => setOpen(s.setup_required), () => setOpen(false))
    }, [])

    const asked = params.get('step') as Step | null
    const step: Step = token && asked && asked !== 'account' ? asked : token ? 'provider' : 'account'
    const go = (s: Step) => setParams({ step: s }, { replace: true })

    if (open === null) return null
    if (!open && !token) return <Navigate to="/login" replace />

    return (
        <div className="flex flex-col gap-20">
            <Stepper
                look="compact"
                aria-label="Setup steps"
                steps={[
                    { label: 'Account', state: step === 'account' ? 'current' : 'done' },
                    { label: 'Provider', state: step === 'provider' ? 'current' : step === 'agent' ? 'done' : 'upcoming' },
                    { label: 'First agent', state: step === 'agent' ? 'current' : 'upcoming' },
                ]}
            />
            {step === 'account' && <AccountStep onDone={() => go('provider')} />}
            {step === 'provider' && <ProviderStep onDone={() => go('agent')} />}
            {step === 'agent' && <AgentStep />}
        </div>
    )
}

function AccountStep({ onDone }: { onDone: () => void }) {
    const load = useSession((s) => s.load)
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [confirm, setConfirm] = useState('')
    const [touched, setTouched] = useState(false)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()

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
            onDone()
        } catch (err) {
            setError(err instanceof Error ? err.message : 'The account could not be created.')
        } finally {
            setBusy(false)
        }
    }

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
            <p className="type-small text-faint">Next you’ll add a provider key and create your first agent.</p>
        </form>
    )
}

function ProviderStep({ onDone }: { onDone: () => void }) {
    const [providers, setProviders] = useState<ProviderSummary[]>([])
    const [connectedCount, setConnectedCount] = useState<number>()
    const [connecting, setConnecting] = useState<ProviderSummary>()

    const load = () => {
        void allProviders().then((p) => setProviders(p ?? []))
        void connectedProviders().then((c) => setConnectedCount(c?.length ?? 0), () => setConnectedCount(0))
    }
    useEffect(load, [])

    const ollama = providers.find((p) => p.id === 'ollama')
    const popular = providers.filter((p) => p.popular && p.fields.length > 0 && p.id !== 'ollama').slice(0, 6)

    if (connecting) {
        return (
            <div className="flex flex-col gap-16">
                <h1 className="type-heading text-fg">Connect {connecting.name}</h1>
                <ConnectForm
                    provider={connecting}
                    onCancel={() => setConnecting(undefined)}
                    onDone={() => {
                        setConnecting(undefined)
                        load()
                    }}
                />
            </div>
        )
    }

    return (
        <div className="flex flex-col gap-16">
            <div className="flex flex-col gap-6">
                <h1 className="type-display text-fg">Add a provider</h1>
                <p className="type-ui-para text-muted">Your agents need a model to think with. Connect one now; you can add more in Settings.</p>
            </div>
            <div className="grid grid-cols-1 gap-10 sm:grid-cols-2">
                {popular.map((p) => (
                    <ProviderTile key={p.id} provider={p.id} name={p.name} onClick={() => setConnecting(p)} />
                ))}
            </div>
            {ollama && (
                <Callout
                    title="No API key?"
                    action={
                        <Button onClick={() => setConnecting(ollama)}>Use Ollama</Button>
                    }
                >
                    Ollama runs open models on this machine.
                </Callout>
            )}
            {connectedCount ? <p className="type-small text-muted">{connectedCount} provider{connectedCount === 1 ? '' : 's'} connected.</p> : null}
            <Button variant="primary" size="lg" block disabled={!connectedCount} onClick={onDone}>
                Continue
            </Button>
        </div>
    )
}

function AgentStep() {
    const navigate = useNavigate()
    const [name, setName] = useState('')
    const [provider, setProvider] = useState('')
    const [model, setModel] = useState('')
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()

    // Start on a provider the person connected.
    useEffect(() => {
        void connectedProviders().then((c) => c?.[0] && setProvider((p) => p || c[0].id))
    }, [])

    const id = slugOf(name)
    const ready = name.trim() !== '' && id !== '' && provider !== '' && model !== ''

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                if (!ready) return
                setBusy(true)
                setError(undefined)
                try {
                    await createAgent(toInput({ ...emptyForm(), agentId: id, name: name.trim(), provider, model }))
                    await useAgents.getState().refresh()
                    navigate(`/${id}/chat`, { replace: true })
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'The agent could not be created.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            <div className="flex flex-col gap-6">
                <h1 className="type-display text-fg">Meet your first agent</h1>
                <p className="type-ui-para text-muted">Give it a name and a model. You can change everything later in Config.</p>
            </div>
            {error && <Notice tone="failed" title="The agent could not be created">{error}</Notice>}
            <TextField label="Name" size="lg" value={name} onChange={(e) => setName(e.target.value)} helper={id ? `Agent ID: ${id}` : undefined} autoFocus />
            <ModelSelect provider={provider} model={model} onChange={(c) => (setProvider(c.provider), setModel(c.model))} />
            <Button type="submit" variant="primary" size="lg" block loading={busy} disabled={!ready}>
                Create agent and start chatting
            </Button>
        </form>
    )
}
