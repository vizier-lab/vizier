import type { ReactNode } from 'react'

type Props = {
    children: ReactNode
    /** The right column: a preview, self-management, activity. */
    aside?: ReactNode
}

/** The design's page body: content on the left, a 340px column on the right (stacked on a phone). */
export function TwoColumn({ children, aside }: Props) {
    return (
        <div className="grid grid-cols-1 items-start gap-24 lg:grid-cols-[minmax(0,1fr)_340px]">
            <div className="flex min-w-0 flex-col gap-24">{children}</div>
            {aside && <div className="flex min-w-0 flex-col gap-24">{aside}</div>}
        </div>
    )
}
