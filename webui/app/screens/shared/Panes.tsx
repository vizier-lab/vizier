import type { ReactNode } from 'react'
import { useMobile } from '../../lib/useMobile'

type Props = {
    /** The main area: what the page is about. */
    children: ReactNode
    /** The side pane: a file explorer, a history, details. */
    pane: ReactNode
    paneLabel: string
    /** Which side the pane sits on. Chat's details are on the right; an explorer is on the left. */
    side?: 'left' | 'right'
    /** The pane's width in px. */
    paneWidth?: 280 | 320 | 360 | 400
    /** The main area scrolls with a 24px gutter; `bare` leaves both to it (a graph, an editor). */
    bare?: boolean
    /** On a phone the panes stack in one scroll: `pane-first` puts the pane on top. */
    phoneOrder?: 'main-first' | 'pane-first'
}

const WIDTH = { 280: 'w-280', 320: 'w-320', 360: 'w-360', 400: 'w-400' } as const

/**
 * A page body split like Chat's: the main area and a pane, each filling the page's height and
 * scrolling on its own, with one rule between them and no cards around either. On a phone they
 * stack in one scroll.
 */
export function Panes({ children, pane, paneLabel, side = 'right', paneWidth = 320, bare, phoneOrder = 'main-first' }: Props) {
    const mobile = useMobile()
    const main = <div className={bare ? 'flex min-h-0 min-w-0 flex-1 flex-col' : 'flex min-w-0 flex-1 flex-col gap-24 px-16 py-20 sm:px-24 sm:py-24'}>{children}</div>
    if (mobile) {
        const aside = (
            <aside aria-label={paneLabel} className="flex flex-col gap-24 border-border px-16 py-20 [&>*]:min-w-0">
                {pane}
            </aside>
        )
        // A main area that has nothing for a phone (Memory's graph) leaves just the pane.
        if (!children) return <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">{aside}</div>
        return (
            <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
                {phoneOrder === 'pane-first' ? (
                    <>
                        {aside}
                        <div className="border-t border-border">{main}</div>
                    </>
                ) : (
                    <>
                        {main}
                        <div className="border-t border-border">{aside}</div>
                    </>
                )}
            </div>
        )
    }
    const aside = (
        <aside
            aria-label={paneLabel}
            className={`flex ${WIDTH[paneWidth]} shrink-0 flex-col gap-24 overflow-x-hidden overflow-y-auto border-border p-16 [&>*]:min-w-0 ${side === 'left' ? 'border-r' : 'border-l'}`}
        >
            {pane}
        </aside>
    )
    return (
        <div className="flex min-h-0 flex-1">
            {side === 'left' && aside}
            <div className={`flex min-h-0 min-w-0 flex-1 flex-col ${bare ? '' : 'overflow-y-auto'}`}>{main}</div>
            {side === 'right' && aside}
        </div>
    )
}
