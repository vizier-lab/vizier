// The one-line page of the viewer: a message and nothing else. No router, no API client, so the
// single-file template (`viewer.html`) stays lean; `routes/share.tsx` wraps it with a sign-in link.
import { Notice } from '@vizier/design'

export const DAMAGED = 'This file is damaged or was edited, so it was not opened.'

export function NeutralNotice({ message }: { message: string }) {
    return (
        <main className="mx-auto flex min-h-screen max-w-md flex-col items-center justify-center gap-16 px-24 text-center">
            <Notice tone="info" title={message} />
        </main>
    )
}

/** Shown at the top of an offline file (FR-035): the file has left Vizier, which can neither vouch for it nor take it back. */
export const OFFLINE_BANNER = 'Offline copy saved from Vizier. Vizier cannot verify it, limit who opens it or revoke it, and it may have been edited since.'
