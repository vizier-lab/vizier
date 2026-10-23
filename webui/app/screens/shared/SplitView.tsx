import { SlideOver, SlideOverPanel } from '@vizier/design'
import type { ReactNode } from 'react'
import { useWide } from '../../lib/useWide'

type Props = {
    /** The list. */
    children: ReactNode
    /** The chosen item's detail; absent when nothing is chosen. */
    detail?: ReactNode
    title: ReactNode
    footer?: ReactNode
    onClose: () => void
}

/**
 * A list with the chosen item's detail beside it, as the design has Skills and Tasks. Where there
 * is no room for both (under 1280px) the detail slides over the list instead.
 */
export function SplitView({ children, detail, title, footer, onClose }: Props) {
    const wide = useWide()
    if (!wide) {
        return (
            <>
                {children}
                <SlideOver open={!!detail} title={title} footer={footer} onClose={onClose}>
                    {detail}
                </SlideOver>
            </>
        )
    }
    if (!detail) return <>{children}</>
    return (
        <div className="grid grid-cols-[minmax(0,1fr)_minmax(380px,440px)] items-start gap-24">
            <div className="min-w-0">{children}</div>
            <SlideOverPanel
                title={title}
                footer={footer}
                onClose={onClose}
                className="sticky top-0 h-auto max-h-[calc(100vh-8rem)] w-full rounded-xl border border-border-strong shadow-none"
            >
                {detail}
            </SlideOverPanel>
        </div>
    )
}
