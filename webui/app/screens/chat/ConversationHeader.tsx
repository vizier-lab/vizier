import { Button, TopicHeader } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router'
import type { ConversationItem } from '../../api/endpoints/conversations'
import { conversationHref } from '../../lib/conversations'
import { ConversationPicker } from './ConversationPicker'

type Props = {
    agentId: string
    title: string
    /** "webui · topic 3 of 5". */
    meta: ReactNode
    current?: { topic?: string; channel?: string }
    liveTopics?: string[]
    /** The jobs pill and the panel switch. */
    trailing?: ReactNode
}

/** The top of a conversation: its title opens the conversation picker, and "New topic" starts a fresh one. */
export function ConversationHeader({ agentId, title, meta, current, liveTopics, trailing }: Props) {
    const navigate = useNavigate()
    const [open, setOpen] = useState(false)
    const fresh = () => navigate(`/${agentId}/chat/${nanoid(10)}`)

    return (
        <header className="flex items-center gap-12 border-b border-border px-24 py-12">
            <ConversationPicker
                agentId={agentId}
                open={open}
                onOpenChange={setOpen}
                trigger={<TopicHeader topic={title} meta={meta} />}
                current={current}
                liveTopics={liveTopics}
                onSelect={(c: ConversationItem) => navigate(conversationHref(agentId, c))}
                onNew={fresh}
            />
            <div className="flex-1" />
            {trailing}
            <Button icon="plus" size="sm" onClick={fresh}>
                New topic
            </Button>
        </header>
    )
}
