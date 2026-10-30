import { useEffect, useState } from 'react'
import { listConversations, type ConversationItem } from '../../api/endpoints/conversations'

const POLL_MS = 15_000
/** How many more a click on Show more brings in. */
export const SESSIONS_STEP = 10

/**
 * The agent's most recent conversations for the sidebar. It looks again when the page changes (a
 * conversation just started shows up) and on a slow timer while the tab is visible; a failed look
 * keeps what it had. It reads `count` conversations (those matching `q`, when it has text), following the server's cursor when one page isn't enough.
 */
export function useSessions(agentId: string | undefined, refreshKey: string, count = SESSIONS_STEP, q = '') {
    const [state, setState] = useState<{ agentId?: string; items: ConversationItem[]; total: number }>({ items: [], total: 0 })

    useEffect(() => {
        if (!agentId) return
        const ctrl = new AbortController()
        const load = async () => {
            try {
                let page = await listConversations(agentId, { q, limit: count }, ctrl.signal)
                const items = [...page.items]
                while (page.next && items.length < count) {
                    page = await listConversations(agentId, { q, limit: count - items.length, cursor: page.next }, ctrl.signal)
                    items.push(...page.items)
                }
                setState({ agentId, items: items.slice(0, count), total: page.total })
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
    }, [agentId, refreshKey, count, q])

    // Another agent's list is never shown under this one while it loads.
    return state.agentId === agentId ? state : { items: [], total: 0 }
}
