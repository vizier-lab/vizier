import { useEffect } from 'react'
import { useNavigate } from 'react-router'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'
import { useToasts } from '../../stores/toasts'

/**
 * Turns newly arrived notifications into timed toasts. Rendered by the shell so it runs on
 * every screen; it draws nothing itself — `ToastRegion` shows what it pushes.
 */
export function NotificationToasts() {
    const arrived = useNotifications((s) => s.arrived)
    const takeArrived = useNotifications((s) => s.takeArrived)
    const markRead = useNotifications((s) => s.markRead)
    const openPanel = useNotifications((s) => s.openPanel)
    const agents = useAgents((s) => s.agents)
    const navigate = useNavigate()

    useEffect(() => {
        if (arrived.length === 0) return
        const items = takeArrived()
        // StrictMode runs an effect twice with the same closure: the second drain is empty.
        if (items.length === 0) return
        const push = useToasts.getState().push
        const nameOf = (id: string) =>
            agents.find((a) => a.agent_id === id)?.name ?? id

        // A burst is one toast; its action opens the panel rather than guessing a row.
        if (items.length > 1) {
            push(
                {
                    tone: 'info',
                    title: `${items.length} new notifications`,
                    message: items[0]?.text,
                    onSelect: openPanel,
                },
                6000
            )
            return
        }

        const item = items[0]
        push(
            {
                tone:
                    item.tone === 'failed'
                        ? 'error'
                        : item.tone === 'success'
                          ? 'success'
                          : 'info',
                title: nameOf(item.agent_id),
                message: item.text,
                onSelect: () => {
                    if (!item.read) void markRead(item.id, true)
                    navigate(item.href)
                },
            },
            item.tone === 'failed' ? 8000 : 6000
        )
    }, [arrived, agents, markRead, navigate, openPanel, takeArrived])

    return null
}
