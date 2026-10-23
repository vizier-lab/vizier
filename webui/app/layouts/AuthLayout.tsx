import { LedBoard, Wordmark } from '@vizier/design'
import { useEffect, useState } from 'react'
import { Outlet } from 'react-router'
import { serverStatus, type ServerStatus } from '../api/endpoints/auth'

const CAPABILITIES = [
    { title: 'Remembers', text: 'memory you can read and edit' },
    { title: 'Dreams', text: 'reflects while you sleep' },
    { title: 'Serves', text: 'Discord, Telegram, Web' },
]

/** The Login and Onboarding frame: the LED board with the brand lockup, and the form on the right. */
export default function AuthLayout() {
    const [status, setStatus] = useState<ServerStatus | null | 'down'>(null)

    useEffect(() => {
        serverStatus().then(setStatus, () => setStatus('down'))
    }, [])

    return (
        <div className="grid min-h-screen grid-cols-1 bg-ground text-fg lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <aside className="relative hidden bg-ground lg:block" aria-label="Vizier">
                <LedBoard className="absolute inset-0" />
                <div className="absolute inset-x-0 bottom-0 flex flex-col gap-16 px-48 pb-48">
                    <Wordmark size={56} />
                    <p className="type-heading text-fg">A digital steward for Your Majesty.</p>
                    <p className="max-w-sm type-ui-para text-muted">AI agents that remember who they are, and who you are. Hosted by you.</p>
                    <dl className="flex gap-32">
                        {CAPABILITIES.map((c) => (
                            <div key={c.title} className="flex flex-col gap-2">
                                <dt className="type-ui-medium text-fg">{c.title}</dt>
                                <dd className="type-small text-faint">{c.text}</dd>
                            </div>
                        ))}
                    </dl>
                </div>
            </aside>
            <main className="flex min-h-screen flex-col bg-surface px-32 py-32">
                <div className="flex flex-1 items-center justify-center">
                    <div className="w-full max-w-sm">
                        <div className="mb-32 lg:hidden">
                            <Wordmark size={36} />
                        </div>
                        <Outlet />
                    </div>
                </div>
                <p className="flex items-center gap-8 type-mono-xs text-faint" role="status">
                    {status === null ? null : status === 'down' ? (
                        <>
                            <span aria-hidden className="size-6 rounded-pill bg-failed" />
                            Server unreachable
                        </>
                    ) : (
                        <>
                            <span aria-hidden className="size-6 rounded-pill bg-live" />
                            Server online · {typeof location === 'undefined' ? '' : location.host} · v{status.version}
                        </>
                    )}
                </p>
            </main>
        </div>
    )
}
