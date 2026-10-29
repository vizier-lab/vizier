import { useEffect, useState } from 'react'
import { listConversations, type ConversationItem } from '../../api/endpoints/conversations'

const POLL_MS = 15_000
const PAGE = 10

/**
 * The agent's most recent conversations for the sidebar. It looks again when the page changes (a
 * conversation just started shows up) and on a slow timer while the tab is visible; a failed look
 * keeps what it had.
 */
export function useSessions(agentId: string | undefined, refreshKey: string) {
    const [state, setState] = useState<{ agentId?: string; items: ConversationItem[]; total: number }>({ items: [], total: 0 })

    useEffect(() => {
        if (!agentId) return
        const ctrl = new AbortController()
        const load = () =>
            listConversations(agentId, { limit: PAGE }, ctrl.signal)
                .then((page) => setState({ agentId, items: page.items, total: page.total }))
                .catch(() => {})
        const soon = setTimeout(load, 200)
        const id = setInterval(() => document.visibilityState === 'visible' && load(), POLL_MS)
        return () => {
            ctrl.abort()
            clearTimeout(soon)
            clearInterval(id)
        }
    }, [agentId, refreshKey])

    // Another agent's list is never shown under this one while it loads.
    return state.agentId === agentId ? state : { items: [], total: 0 }
}
