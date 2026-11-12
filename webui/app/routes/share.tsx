// /share/:shareId — a shared conversation, read only. Public: the server decides who may open it.
// What comes back is hostile input, read with `parseSnapshot`. The tab title is set as text.
import { Button, Notice, Skeleton } from '@vizier/design'
import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router'
import { getShare } from '../api/shares'
import { ApiError } from '../api/client'
import { parseSnapshot, type Snapshot } from '../lib/share/schema'
import { SnapshotView } from '../viewer/SnapshotView'

export type ShareView =
    | { state: 'loading' }
    | { state: 'ok'; snapshot: Snapshot }
    | { state: 'unavailable'; signIn: boolean; message?: string }
    | { state: 'damaged'; message: string }
    | { state: 'needs_newer'; message: string }

/** What the answer (or the failure) of the fetch means for the page. */
export function viewFor(result: { data: unknown } | { error: unknown }): ShareView {
    if ('error' in result) {
        const e = result.error
        const is401 = e instanceof ApiError && e.status === 401
        const is429 = e instanceof ApiError && e.status === 429
        return {
            state: 'unavailable',
            signIn: is401,
            ...(is429 ? { message: RATE_LIMITED } : {}),
        }
    }
    const data = result.data as { snapshot?: unknown } | null
    const parsed = parseSnapshot(data?.snapshot)
    if (parsed.ok) return { state: 'ok', snapshot: parsed.snapshot }
    return { state: parsed.reason === 'needs_newer' ? 'needs_newer' : 'damaged', message: parsed.message }
}

export const UNAVAILABLE = "This link isn't available."
export const RATE_LIMITED = 'Too many requests. Try again later.'
export const DAMAGED = 'This file is damaged or was edited, so it was not opened.'

export function Neutral({ message, signIn, shareId }: { message: string; signIn?: boolean; shareId?: string }) {
    const returnTo = shareId ? `/share/${encodeURIComponent(shareId)}` : undefined
    return (
        <main className="mx-auto flex min-h-screen max-w-md flex-col items-center justify-center gap-16 px-24 text-center">
            <Notice tone="info" title={message} />
            {signIn && (
                <Link
                    to={returnTo ? `/login?returnTo=${encodeURIComponent(returnTo)}` : '/login'}
                    state={returnTo ? { from: returnTo } : undefined}
                >
                    <Button variant="primary">Sign in</Button>
                </Link>
            )}
        </main>
    )
}

export default function Share() {
    const { shareId = '' } = useParams()
    const [view, setView] = useState<ShareView>({ state: 'loading' })

    useEffect(() => {
        const ctl = new AbortController()
        setView({ state: 'loading' })
        getShare(shareId, ctl.signal).then(
            (data) => !ctl.signal.aborted && setView(viewFor({ data })),
            (error) => !ctl.signal.aborted && setView(viewFor({ error }))
        )
        return () => ctl.abort()
    }, [shareId])

    // The page asks not to be listed by search engines (spec Clarifications); previews get no title.
    useEffect(() => {
        const meta = document.createElement('meta')
        meta.name = 'robots'
        meta.content = 'noindex, nofollow, noarchive'
        document.head.appendChild(meta)
        return () => meta.remove()
    }, [])

    const title = view.state === 'ok' ? view.snapshot.title : ''
    useEffect(() => {
        // Text only: `document.title` never parses markup.
        document.title = title ? `${title.slice(0, 120)} · Vizier` : 'Vizier'
    }, [title])

    switch (view.state) {
        case 'loading':
            return (
                <main className="mx-auto max-w-3xl px-24 py-32" aria-busy="true">
                    <Skeleton />
                </main>
            )
        case 'ok':
            return <SnapshotView snapshot={view.snapshot} />
        case 'unavailable':
            return <Neutral message={view.message ?? UNAVAILABLE} signIn={view.signIn} shareId={shareId} />
        case 'needs_newer':
            return <Neutral message={view.message} />
        default:
            return <Neutral message={DAMAGED} />
    }
}
