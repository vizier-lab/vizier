// The one-line page of the viewer: a message and nothing else. No router, no API client, so the
// single-file template (`viewer.html`) stays lean; `routes/share.tsx` wraps it with a sign-in link.
import { Notice } from '@vizier/design'

export const DAMAGED = 'This file is damaged or was edited, so it was not opened.'

export function NeutralNotice({ message }: { message: string }) {
    return (
        <main aria-label="Shared conversation" className="mx-auto flex min-h-screen max-w-md flex-col items-center justify-center gap-16 px-24 text-center">
            <h1 className="sr-only">Shared conversation</h1>
            <Notice tone="info" title={message} />
        </main>
    )
}

/** Shown at the top of an offline file (FR-035): the file has left Vizier, which can neither vouch for it nor take it back. */
export const OFFLINE_BANNER = 'Offline copy saved from Vizier. Vizier cannot verify it, limit who opens it or revoke it, and it may have been edited since.'

/** T088: the same banner names whoever saved the file, when they chose to show their name (FR-007). */
export function offlineBanner(author?: string): string {
    const name = typeof author === 'string' ? author.trim() : ''
    return name ? `${OFFLINE_BANNER} Saved by ${name}.` : OFFLINE_BANNER
}
