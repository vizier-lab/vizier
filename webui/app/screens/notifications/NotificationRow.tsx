import { IconButton, Sigil } from '@vizier/design'
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
 * One notification, as a card: the agent's sigil, its name, when it happened and the stored
 * summary. Unread is carried by the summary's tone and a hidden word, not a dot. The
 * mark-read control is a sibling of the card's link (never nested in it) but inside the card.
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
              : item.read
                ? 'text-muted'
                : 'text-soft'

    return (
        <div className="group/row flex w-full min-w-0 items-start gap-10 rounded-lg border border-border bg-surface px-12 py-10 transition-colors hover:border-border-hover">
            <Sigil agentId={item.agent_id} size="md" label={name} />
            <button
                type="button"
                onClick={open}
                className="flex min-w-0 flex-1 flex-col gap-3 text-left"
            >
                <span className="flex min-w-0 items-center gap-8">
                    <span className="min-w-0 flex-1 truncate type-ui-medium text-fg">
                        {name}
                    </span>
                    <time
                        dateTime={item.at}
                        className="shrink-0 type-mono-xs whitespace-nowrap text-faint"
                    >
                        {feedTime(item.at)}
                    </time>
                </span>
                <span className={`type-ui-para ${tone}`}>
                    {item.text}
                    {!item.read && <span className="sr-only"> Unread</span>}
                </span>
            </button>
            <IconButton
                variant="ghost"
                size="sm"
                icon={item.read ? 'restore' : 'check'}
                aria-label={item.read ? 'Mark as unread' : 'Mark as read'}
                className="opacity-100 md:opacity-0 md:group-hover/row:opacity-100 md:focus-visible:opacity-100"
                onClick={() => void markRead(item.id, !item.read)}
            />
        </div>
    )
}
