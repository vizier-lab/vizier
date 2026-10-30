import { useLocation } from 'react-router'

/** What a page keeps in the navigation state so the page it opened can lead back to it. */
export type ReturnState = { from?: string }

/**
 * The state to hand `navigate`/`Link` when leaving this page for a detail page: where this page
 * stood, with its search, filter, view and date. Going back then restores them, not a bare list.
 */
export function useHere(): ReturnState {
    const { pathname, search } = useLocation()
    return { from: pathname + search }
}

/** Where "back" leads: the page that sent us here, if it sent a path of this app, else `fallback`. */
export function useReturnTo(fallback: string): string {
    const from = (useLocation().state as ReturnState | null)?.from
    return typeof from === 'string' && from.startsWith('/') && !from.startsWith('//') ? from : fallback
}

/** The state to pass on when moving between detail pages (view → edit → saved), so the way back survives. */
export function useCarry(): ReturnState | undefined {
    const from = (useLocation().state as ReturnState | null)?.from
    return from ? { from } : undefined
}
