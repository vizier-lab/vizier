import { useEffect } from 'react'
import { useNavigate } from 'react-router'
import { transport } from '../../api/transport'
import { shouldNotifyOs } from '../../lib/desktop'
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
        // The desktop window is hidden or unfocused: tell the OS instead of drawing a toast
        // nobody sees. Selecting the notification opens `href` (or the panel).
        if (transport.authMode === 'desktop' && shouldNotifyOs(document.visibilityState, document.hasFocus())) {
            const nameFor = (id: string) => agents.find((a) => a.agent_id === id)?.name ?? id
            if (items.length > 1) {
                void transport.notify({ title: `${items.length} new notifications`, body: items[0]?.text ?? '', href: '/notifications' })
            } else {
                void transport.notify({ title: nameFor(items[0].agent_id), body: items[0].text, href: items[0].href })
            }
            return
        }
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
