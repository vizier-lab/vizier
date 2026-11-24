import { IconButton } from '@vizier/design'
import { useCallback, useMemo } from 'react'
import { entryLabel } from '../../lib/notifications'
import { useMobile } from '../../lib/useMobile'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'

/**
 * The notification bell in a page header: the unread count as a badge, and a
 * separate dot while an agent is setting up. The popup hangs from this control, which is
 * its anchor and also closes it. The accessible name states both in words.
 */
export function NotificationsBell() {
    const unread = useNotifications((s) => s.counts.unread)
    const open = useNotifications((s) => s.open)
    const togglePanel = useNotifications((s) => s.togglePanel)
    const setAnchor = useNotifications((s) => s.setAnchor)
    // The newest bell on screen is the anchor; one that unmounts only clears what is its own.
    const anchorRef = useCallback(
        (el: HTMLSpanElement | null) => {
            if (el) setAnchor(el)
            else if (useNotifications.getState().anchor?.isConnected === false) setAnchor(null)
        },
        [setAnchor]
    )
    // A phone's top bar gives every control a thumb's 44px.
    const mobile = useMobile()
    const setups = useAgents((s) => s.setups)
    const busy = useMemo(
        () =>
            Object.values(setups).some(
                (s) => s.phase === 'starting' || s.phase === 'downloading'
            ),
        [setups]
    )

    return (
        <span ref={anchorRef} className="inline-flex">
            <IconButton
                variant="ghost"
                size={mobile ? 'lg' : 'md'}
                icon="bell"
                count={unread}
                busy={busy}
                selected={open}
                aria-label={entryLabel(unread, busy)}
                aria-expanded={open}
                aria-haspopup="dialog"
                onClick={togglePanel}
            />
        </span>
    )
}
