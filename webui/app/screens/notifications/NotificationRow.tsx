import { NotificationItem as Item } from '@vizier/design'
import { useNavigate } from 'react-router'
import type { NotificationItem } from '../../api/endpoints/notifications'
import { feedTime } from '../../lib/format'
import { useAgents } from '../../stores/agents'
import { useNotifications } from '../../stores/notifications'

type Props = {
    item: NotificationItem
    /** Close the popup before moving to the destination. */
    onNavigate: () => void
}

/**
 * One notification, drawn by the design's `NotificationItem`. A row with no `title` or `excerpt`
 * (older than the feature, or a conversation that isn't yours) still reads fully from its
 * summary, with the agent's name standing in for the title.
 */
export function NotificationRow({ item, onNavigate }: Props) {
    const navigate = useNavigate()
    const agents = useAgents((s) => s.agents)
    const markRead = useNotifications((s) => s.markRead)
    // A Workspace row is about the Workspace, not an agent: its title (`ops #general`) names it.
    const inWorkspace = item.category === 'workspace_message'
    const name = inWorkspace
        ? (item.title ?? 'Workspace')
        : (agents.find((a) => a.agent_id === item.agent_id)?.name ?? item.agent_id)

    return (
        <Item
            agentId={item.agent_id}
            agentName={name}
            title={inWorkspace ? undefined : item.title}
            text={item.text}
            excerpt={item.excerpt}
            time={feedTime(item.at)}
            dateTime={item.at}
            read={item.read}
            tone={item.tone}
            onOpen={() => {
                if (!item.read) void markRead(item.id, true)
                onNavigate()
                navigate(item.href)
            }}
            onToggleRead={() => void markRead(item.id, !item.read)}
        />
    )
}
