import { FadeRule } from '../shared/SideCard'
import {
    IconButton,
    MobileTopBar,
    SlideOver,
    Tooltip,
    TopicHeader,
    type MobileTopBarAgent,
} from '@vizier/design'
import { nanoid } from 'nanoid'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { sharesOfConversation } from '../../api/shares'
import { agentState } from '../../api/endpoints/agents'
import type { ConversationItem } from '../../api/endpoints/conversations'
import { canShareConversations } from '../../lib/access'
import { detailsPath } from '../../lib/agentDetails'
import { conversationHref } from '../../lib/conversations'
import { assetSrc } from '../../lib/fileUrls'
import { useMobile } from '../../lib/useMobile'
import { useAgents } from '../../stores/agents'
import { useNav } from '../../stores/nav'
import { useSession } from '../../stores/session'
import { NotificationsBell } from '../shared/NotificationsBell'
import { AgentConversations } from '../shell/AgentConversations'
import { ConversationPicker } from './ConversationPicker'
import { ShareDialog, type ExistingLinks } from './ShareDialog'

/** What the Share button needs; absent where a conversation cannot be shared (a channel read-out). */
export type ShareTarget = {
    topicId: string
    agent: { owner_id?: string } | undefined
    agentName: string
    /** No messages yet: the button is disabled. */
    empty: boolean
}

export const EMPTY_SHARE_REASON = 'Nothing to share yet'

/**
 * The conversation's live link shares, for the share dialog's list. Read here, once per
 * conversation, so opening the dialog itself stores and asks nothing; `reload` re-reads after
 * a link is created or revoked.
 */
export function useLiveShares(agentId: string, topicId: string | undefined, enabled: boolean): ExistingLinks {
    const [state, setState] = useState<Omit<ExistingLinks, 'reload'>>({ links: [], loading: false, error: null })
    const [version, setVersion] = useState(0)
    useEffect(() => {
        if (!enabled || !topicId) {
            setState({ links: [], loading: false, error: null })
            return
        }
        const ctl = new AbortController()
        setState((s) => ({ ...s, loading: true }))
        sharesOfConversation(agentId, topicId, ctl.signal).then(
            (links) => !ctl.signal.aborted && setState({ links, loading: false, error: null }),
            (e) =>
                !ctl.signal.aborted &&
                setState({ links: [], loading: false, error: e instanceof Error ? e.message : 'The links could not be read.' })
        )
        return () => ctl.abort()
    }, [agentId, topicId, enabled, version])
    return { ...state, reload: () => setVersion((v) => v + 1) }
}

type Props = {
    share?: ShareTarget
    agentId: string
    title: string
    /** "webui · topic 3 of 5". */
    meta: ReactNode
    current?: { topic?: string; channel?: string }
    liveTopics?: string[]
    /** The jobs pill and the panel switch. */
    trailing?: ReactNode
    /** On a phone, what acts on this conversation (new, delete), at the end of the topic row. */
    mobileTrailing?: ReactNode
    /** The agent and how it is doing, for the phone bar. */
    agentName?: string
    status?: {
        state: MobileTopBarAgent['state']
        detail?: ReactNode
    }
}

/**
 * The top of a conversation: its title opens the conversation picker, which also starts a fresh one.
 * On a phone (spec 056) the bar carries the agent, which switches agent from its name and opens its
 * details from the side, and the conversation is a second row that opens the topics as a sheet.
 */
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
    share,
}: Props) {
    useSession((s) => s.user) // the permission is read from the session; re-evaluate when it changes
    const [sharing, setSharing] = useState(false)
    // The control that opened the dialog. A native <dialog> restores focus itself only while the
    // element it remembers is still the one in the DOM; React unmounts ours on close, so the
    // keyboard path back would land on <body>. Remembered from the click, not a ref prop.
    const shareTrigger = useRef<HTMLElement | null>(null)
    const canShare = !!share && canShareConversations(share.agent)
    const existing = useLiveShares(agentId, share?.topicId, canShare)
    const shareButton = canShare ? (
        <>
            {share.empty ? (
                <Tooltip label={EMPTY_SHARE_REASON} side="bottom">
                    <span>
                        <IconButton icon="link" aria-label={`Share this conversation. ${EMPTY_SHARE_REASON}`} disabled />
                    </span>
                </Tooltip>
            ) : (
                <IconButton
                    icon="link"
                    aria-label="Share this conversation"
                    onClick={(event) => {
                        shareTrigger.current = event.currentTarget
                        setSharing(true)
                    }}
                />
            )}
            {sharing && (
                <ShareDialog
                    open
                    onClose={() => {
                        setSharing(false)
                        requestAnimationFrame(() => shareTrigger.current?.focus())
                    }}
                    agentId={agentId}
                    topicId={share.topicId}
                    title={title}
                    agentName={share.agentName}
                    existing={existing}
                />
            )}
        </>
    ) : null

    const mobile = useMobile()
    const setNavOpen = useNav((s) => s.setOpen)
    const setSwitcherOpen = useNav((s) => s.setSwitcherOpen)
    const { agents, health } = useAgents()
    const navigate = useNavigate()
    const { pathname, search } = useLocation()
    const [open, setOpen] = useState(false)
    // Choosing a conversation in the sheet navigates; the sheet closes behind it.
    useEffect(() => setOpen(false), [pathname, search])
    const fresh = () => navigate(`/${agentId}/chat/${nanoid(10)}`)

    const select = (c: ConversationItem) => {
        setOpen(false)
        navigate(conversationHref(agentId, c))
    }

    if (mobile) {
        const agent = agents.find((a) => a.agent_id === agentId)
        return (
            <>
                <MobileTopBar
                    onMenu={() => setNavOpen(true)}
                    agent={{
                        agentId,
                        name: agentName ?? agent?.name ?? agentId,
                        imageSrc: assetSrc(agent?.avatar_url),
                        state: status?.state ?? agentState(health[agentId]),
                        detail: status?.detail,
                        onSwitch: () => setSwitcherOpen(true),
                        onDetails: () => navigate(detailsPath(agentId)),
                    }}
                    topic={{
                        title,
                        meta,
                        onTopic: () => setOpen(true),
                        actions: (
                            <div className="flex items-center gap-4">
                                {shareButton}
                                {mobileTrailing}
                            </div>
                        ),
                    }}
                    trailing={<NotificationsBell />}
                />
                <SlideOver open={open} side="bottom" title="Topics" onClose={() => setOpen(false)}>
                    <div className="flex h-[65dvh] min-h-0 flex-col gap-12">
                        <AgentConversations agentId={agentId} />
                    </div>
                </SlideOver>
            </>
        )
    }

    return (
        <header className="relative flex h-76 shrink-0 items-center gap-12 px-24">
            <FadeRule className="pointer-events-none absolute inset-x-0 bottom-0" />
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
            {shareButton}
            <NotificationsBell />
        </header>
    )
}
