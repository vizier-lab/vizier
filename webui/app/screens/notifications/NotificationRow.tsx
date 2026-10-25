import { IconButton, ListItem, Sigil } from '@vizier/design'
import { useNavigate } from 'react-router'
import type { NotificationItem } from '../../api/endpoints/notifications'
import { feedTime } from '../../lib/format'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'

type Props = {
    item: NotificationItem
    /** Close the panel before moving to the destination. */
    onNavigate: () => void
}

/**
 * One notification: a fixed unread gutter, the agent's sigil and name, when it
 * happened, and the stored summary. The mark-read control is the `ListItem`
 * `action`, a sibling of the row's button, so it never navigates.
 */
export function NotificationRow({ item, onNavigate }: Props) {
    const navigate = useNavigate()
    const agents = useAgents((s) => s.agents)
    const markRead = useNotifications((s) => s.markRead)
    const name = agents.find((a) => a.agent_id === item.agent_id)?.name ?? item.agent_id

    const open = () => {
        if (!item.read) void markRead(item.id, true)
        onNavigate()
        navigate(item.href)
    }

    const tone =
        item.tone === 'failed'
            ? 'text-failed-text'
            : item.tone === 'success'
              ? 'text-fg'
              : 'text-muted'

    return (
        <div className="group/row flex min-w-0 items-stretch border-b border-border last:border-b-0">
            {/* A fixed gutter: the dot appears without shifting the row. */}
            <span aria-hidden className="flex w-10 shrink-0 items-start justify-center pt-16">
                {!item.read && <span className="size-6 rounded-pill bg-ink" />}
            </span>
            <div className="min-w-0 flex-1">
                <ListItem
                    shape="flush"
                    align="start"
                    className="w-full"
                    leading={<Sigil agentId={item.agent_id} size="sm" label={name} />}
                    title={name}
                    meta={
                        <time dateTime={item.at}>{feedTime(item.at)}</time>
                    }
                    subtitle={
                        <span className={`type-ui-para ${tone}`}>
                            {item.text}
                            {!item.read && <span className="sr-only"> Unread</span>}
                        </span>
                    }
                    onSelect={open}
                    action={
                        <IconButton
                            variant="ghost"
                            size="sm"
                            icon={item.read ? 'restore' : 'check'}
                            aria-label={item.read ? 'Mark as unread' : 'Mark as read'}
                            className="opacity-100 md:opacity-0 md:group-hover/row:opacity-100 md:focus-visible:opacity-100"
                            onClick={() => void markRead(item.id, !item.read)}
                        />
                    }
                />
            </div>
        </div>
    )
}
