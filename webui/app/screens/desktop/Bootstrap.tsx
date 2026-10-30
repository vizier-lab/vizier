import { Button, Card, Notice, TextField } from '@vizier/design'
import { useState, type ReactNode } from 'react'
import { transport, type DesktopState } from '../../api/transport'
import { bootstrapView, errorText, failureMessage, lockHolder, ownerCandidates } from '../../lib/desktop'

function Frame({ title, description, children }: { title: string; description?: string; children?: ReactNode }) {
    return (
        <main className="flex min-h-dvh items-center justify-center bg-ground p-16">
            <div className="w-full max-w-md">
                <Card title={title} description={description} aria-label={title}>
                    <div className="flex flex-col gap-16">{children}</div>
                </Card>
            </div>
        </main>
    )
}

function Run({ label, run, tone = 'primary' }: { label: string; run: () => Promise<unknown>; tone?: 'primary' | 'secondary' }) {
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    return (
        <div className="flex flex-col gap-12">
            {error && <Notice tone="failed" title="That didn’t work">{error}</Notice>}
            <div>
                <Button
                    variant={tone}
                    loading={busy}
                    onClick={async () => {
                        setBusy(true)
                        setError(undefined)
                        try {
                            await run()
                        } catch (e) {
                            setError(errorText(e, 'Something went wrong.'))
                        } finally {
                            setBusy(false)
                        }
                    }}
                >
                    {label}
                </Button>
            </div>
        </div>
    )
}

function NeedsOwner() {
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    return (
        <Frame title="Welcome to Vizier" description="Create the owner of this workspace. A password is optional; you can add one later.">
            <form
                className="flex flex-col gap-16"
                onSubmit={async (e) => {
                    e.preventDefault()
                    if (!username.trim()) return
                    setBusy(true)
                    setError(undefined)
                    try {
                        await transport.desktop!.setupOwner(username.trim(), password || undefined)
                    } catch (err) {
                        setError(errorText(err, 'The owner could not be created.'))
                    } finally {
                        setBusy(false)
                    }
                }}
            >
                {error && <Notice tone="failed" title="The owner wasn’t created">{error}</Notice>}
                <TextField label="Name" autoComplete="username" value={username} onChange={(e) => setUsername(e.target.value)} />
                <TextField label="Password (optional)" type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
                <div>
                    <Button type="submit" loading={busy} disabled={!username.trim()}>
                        Continue
                    </Button>
                </div>
            </form>
        </Frame>
    )
}

/** Shown instead of the app until the engine is ready (contracts/webui-transport.md). */
export function Bootstrap({ state }: { state: DesktopState | null }) {
    const shell = transport.desktop
    switch (bootstrapView(state)) {
        case 'locked': {
            const holder = lockHolder(state)
            return (
                <Frame title="This workspace is in use" description={holder ? `It is open in another Vizier (${holder}).` : 'It is open in another Vizier.'}>
                    <p className="type-ui text-muted">Close the other one, or open a different workspace.</p>
                    <Run label="Open another workspace" run={() => shell!.openWorkspace()} />
                </Frame>
            )
        }
        case 'needs_owner':
            return <NeedsOwner />
        case 'choose_owner':
            return (
                <Frame title="Who are you?" description="This workspace has several owners. Pick the one this app signs in as.">
                    <ul className="flex flex-col gap-8">
                        {ownerCandidates(state).map((c) => (
                            <li key={c.user_id}>
                                <Run label={c.username} tone="secondary" run={() => shell!.chooseOwner(c.user_id)} />
                            </li>
                        ))}
                    </ul>
                </Frame>
            )
        case 'failed':
            return (
                <Frame title="Vizier couldn’t start">
                    <Notice tone="failed" title="The engine stopped">{failureMessage(state) ?? 'Unknown error.'}</Notice>
                    <Run label="Restart" run={() => shell!.restart()} />
                </Frame>
            )
        default:
            return (
                <Frame title="Starting Vizier…">
                    <p className="type-ui text-muted" role="status">Opening your workspace.</p>
                </Frame>
            )
    }
}
