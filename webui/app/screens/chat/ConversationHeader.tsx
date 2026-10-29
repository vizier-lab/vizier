import {
    MobileTopBar,
    TopicHeader,
    type MobileTopBarProps,
} from '@vizier/design'
import { nanoid } from 'nanoid'
import { useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router'
import type { ConversationItem } from '../../api/endpoints/conversations'
import { conversationHref } from '../../lib/conversations'
import { useMobile } from '../../lib/useMobile'
import { useNav } from '../../stores/nav'
import { NotificationsBell } from '../shared/NotificationsBell'
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
    /** The phone bar's right slot: the jobs count. */
    mobileTrailing?: ReactNode
    /** The agent and how it is doing, for the phone bar. */
    agentName?: string
    status?: {
        state: NonNullable<MobileTopBarProps['chat']>['state']
        detail?: ReactNode
    }
}

/** The top of a conversation: its title opens the conversation picker, which also starts a fresh one. */
export function ConversationHeader({
    agentId,
    title,
    meta,
    current,
    liveTopics,
    trailing,
    mobileTrailing,
    agentName,
    status,
}: Props) {
    const mobile = useMobile()
    const setNavOpen = useNav((s) => s.setOpen)
    const navigate = useNavigate()
    const [open, setOpen] = useState(false)
    const fresh = () => navigate(`/${agentId}/chat/${nanoid(10)}`)

    const select = (c: ConversationItem) => {
        setOpen(false)
        navigate(conversationHref(agentId, c))
    }

    if (mobile) {
        return (
            <div className="relative">
                <MobileTopBar
                    onMenu={() => setNavOpen(true)}
                    chat={{
                        agentId,
                        agentName: agentName ?? agentId,
                        topic: title,
                        state: status?.state ?? 'live',
                        detail: status?.detail,
                        onTopic: () => setOpen((o) => !o),
                    }}
                    trailing={
                        <div className="flex items-center gap-4">
                            {mobileTrailing}
                            <NotificationsBell />
                        </div>
                    }
                />
                {open && (
                    <>
                        <button
                            type="button"
                            aria-label="Close conversations"
                            className="fixed inset-0 z-20 cursor-default"
                            onClick={() => setOpen(false)}
                        />
                        <div className="absolute inset-x-8 top-60 z-30">
                            <ConversationPicker
                                agentId={agentId}
                                open
                                onOpenChange={setOpen}
                                width={Math.min(400, window.innerWidth - 16)}
                                current={current}
                                liveTopics={liveTopics}
                                onSelect={select}
                                onNew={fresh}
                            />
                        </div>
                    </>
                )}
            </div>
        )
    }

    return (
        <header className="relative flex h-76 shrink-0 items-center gap-12 px-24">
            <span aria-hidden className="pointer-events-none absolute inset-x-0 bottom-0 h-px bg-gradient-to-r from-transparent via-border to-transparent" />
            <ConversationPicker
                agentId={agentId}
                open={open}
                onOpenChange={setOpen}
                trigger={<TopicHeader topic={title} meta={meta} />}
                current={current}
                liveTopics={liveTopics}
                onSelect={select}
                onNew={fresh}
            />
            <div className="flex-1" />
            {trailing}
            <NotificationsBell />
        </header>
    )
}
