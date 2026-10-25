import { IconButton } from '@vizier/design'
import { useMemo } from 'react'
import { entryLabel } from '../../lib/notifications'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'

/**
 * The notification bell in a page header: the unread count as a badge, and a
 * separate dot while an agent is setting up. The panel slides in from the right
 * edge, beneath this control. The accessible name states both in words.
 */
export function NotificationsBell() {
    const unread = useNotifications((s) => s.counts.unread)
    const openPanel = useNotifications((s) => s.openPanel)
    const setups = useAgents((s) => s.setups)
    const busy = useMemo(
        () =>
            Object.values(setups).some(
                (s) => s.phase === 'starting' || s.phase === 'downloading'
            ),
        [setups]
    )

    return (
        <IconButton
            variant="ghost"
            icon="bell"
            count={unread}
            busy={busy}
            aria-label={entryLabel(unread, busy)}
            onClick={openPanel}
        />
    )
}
