import type { ReactNode } from 'react'

/** No side panel is narrower than this, so its cards read the same on every page. */
export const SIDE_PANEL_WIDTH = 320

/** A hairline that fades out at both ends, as the header rules do. */
export function FadeRule({ className = '' }: { className?: string }) {
    return <span aria-hidden className={`block h-px bg-[linear-gradient(to_right,transparent,var(--vz-border)_14%,var(--vz-border)_86%,transparent)] ${className}`} />
}

/**
 * A side panel's surface: one rounded card on the page's panel. Put several sections in it with a
 * `FadeRule` between them rather than a card each.
 */
export function SideCard({ children }: { children: ReactNode }) {
    return <div className="flex flex-col gap-20 rounded-xl border border-border bg-sidebar p-18 [&>*]:min-w-0">{children}</div>
}
