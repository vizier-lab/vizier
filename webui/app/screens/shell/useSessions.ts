import { useEffect, useState } from 'react'
import { listConversations, type ConversationItem } from '../../api/endpoints/conversations'

const POLL_MS = 15_000
const PAGE = 10
// A whole list is read a page at a time, up to this many conversations.
const ALL_MAX = 500

/**
 * The agent's most recent conversations for the sidebar. It looks again when the page changes (a
 * conversation just started shows up) and on a slow timer while the tab is visible; a failed look
 * keeps what it had. With `all` it reads the whole list, not just the latest few.
 */
export function useSessions(agentId: string | undefined, refreshKey: string, all = false) {
    const [state, setState] = useState<{ agentId?: string; items: ConversationItem[]; total: number }>({ items: [], total: 0 })

    useEffect(() => {
        if (!agentId) return
        const ctrl = new AbortController()
        const load = async () => {
            try {
                let page = await listConversations(agentId, { limit: all ? 50 : PAGE }, ctrl.signal)
                const items = [...page.items]
                while (all && page.next && items.length < ALL_MAX) {
                    page = await listConversations(agentId, { limit: 50, cursor: page.next }, ctrl.signal)
                    items.push(...page.items)
                }
                setState({ agentId, items, total: page.total })
            } catch {
                /* keeps what it had */
            }
        }
        const soon = setTimeout(load, 200)
        const id = setInterval(() => document.visibilityState === 'visible' && load(), POLL_MS)
        return () => {
            ctrl.abort()
            clearTimeout(soon)
            clearInterval(id)
        }
    }, [agentId, refreshKey, all])

    // Another agent's list is never shown under this one while it loads.
    return state.agentId === agentId ? state : { items: [], total: 0 }
}
