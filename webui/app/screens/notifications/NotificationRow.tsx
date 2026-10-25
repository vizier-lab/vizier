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
 * One notification as a card: a large sigil, the agent and summary, and a read toggle whose
 * eye is also the unread indicator (closed = unread, open = read) to the left of the time.
 * An unread card is outlined in the live emerald; a read one keeps the neutral border.
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
        <div
            className={`group/row flex w-full min-w-0 items-start gap-10 rounded-lg border bg-surface px-12 py-10 transition-colors ${
                item.read
                    ? 'border-border hover:border-border-hover'
                    : 'border-live hover:border-live'
            }`}
        >
            <Sigil agentId={item.agent_id} size="md" label={name} />
            <button
                type="button"
                onClick={open}
                className="flex min-w-0 flex-1 flex-col gap-3 text-left"
            >
                <span className="min-w-0 truncate type-ui-medium text-fg">{name}</span>
                <span className={`type-ui-para ${tone}`}>
                    {item.text}
                    {!item.read && <span className="sr-only"> Unread</span>}
                </span>
            </button>
            <div className="flex shrink-0 items-center gap-4 self-start">
                <IconButton
                    variant="ghost"
                    size="sm"
                    icon={item.read ? 'eye' : 'eye-closed'}
                    aria-label={item.read ? 'Mark as unread' : 'Mark as read'}
                    onClick={() => void markRead(item.id, !item.read)}
                />
                <time
                    dateTime={item.at}
                    className="type-mono-xs whitespace-nowrap text-faint"
                >
                    {feedTime(item.at)}
                </time>
            </div>
        </div>
    )
}
