import { SessionList } from '@vizier/design'
import { nanoid } from 'nanoid'
import { useEffect, useMemo, useState } from 'react'
import { useLocation, useNavigate, useSearchParams } from 'react-router'
import { renameTopic } from '../../api/endpoints/chat'
import { toast } from '../../stores/toasts'
import { DeleteConversation, type DeleteTarget } from '../chat/DeleteConversation'
import { conversationHref, conversationTitle, sidebarSections, sidebarTime } from '../../lib/conversations'
import { unreadAt } from '../../lib/notifications'
import { useNotifications } from '../../stores/notifications'
import { ProjectsSection } from '../projects/ProjectsSection'
import { topicKey } from '../../stores/connection'
import { useTurns } from '../../stores/turns'
import { SESSIONS_STEP, useSessions } from './useSessions'

/**
 * An agent's projects and conversations, grouped by day, with search, paging, rename and delete:
 * the lower half of the desktop agent column, and the whole of the phone's topic sheet.
 */
export function AgentConversations({ agentId }: { agentId: string }) {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const [search] = useSearchParams()
    const [shown, setShown] = useState(SESSIONS_STEP)
    const [sessionQuery, setSessionQuery] = useState('')
    // The conversation being deleted, while its confirmation is open; and a bump that makes the list look again.
    const [deleting, setDeleting] = useState<DeleteTarget | null>(null)
    const [listVersion, setListVersion] = useState(0)
    // The server searches; it is asked once typing pauses.
    const [sessionSearch, setSessionSearch] = useState('')
    useEffect(() => {
        const id = setTimeout(() => setSessionSearch(sessionQuery.trim()), 250)
        return () => clearTimeout(id)
    }, [sessionQuery])

    const { items, total } = useSessions(agentId, `${pathname}#${listVersion}`, shown, sessionSearch)

    const turns = useTurns((s) => s.topics)
    const isLive = (topic: string) => (turns[topicKey(agentId, topic)]?.phase ?? 'idle') !== 'idle'
    const chats = useNotifications((s) => s.chats)

    // The conversation on screen: a WebUI topic by id, a Discord or Telegram one by channel.
    const topicId = pathname.match(/^\/[^/]+\/chat\/([^/]+)$/)?.[1]
    const channel = pathname.endsWith('/chat/read') ? search.get('channel') : null
    const isCurrent = (c: (typeof items)[number]) =>
        channel ? c.channel === channel : c.kind === 'webui' && !!topicId && c.latest_topic === decodeURIComponent(topicId)

    const groups = useMemo(
        () =>
            sidebarSections(items).map((g) => ({
                label: g.label,
                items: g.items.map((c) => ({
                    key: `${c.channel}#${c.latest_topic}`,
                    title: conversationTitle(c.title, c.latest_topic),
                    time: sidebarTime(c.last_activity),
                    live: c.kind === 'webui' && isLive(c.latest_topic),
                    unread: unreadAt(chats, conversationHref(agentId, c)),
                    href: conversationHref(agentId, c),
                    active: isCurrent(c),
                    // Only the WebUI's own conversations: Discord and Telegram ones belong to those platforms.
                    deletable: c.kind === 'webui',
                    renamable: c.kind === 'webui',
                })),
            })),
        [items, turns, chats, topicId, channel, agentId]
    )

    const newConversation = () => navigate(`/${agentId}/chat/${nanoid(10)}`)

    useEffect(() => setShown(SESSIONS_STEP), [agentId, sessionSearch])
    useEffect(() => setSessionQuery(''), [agentId])

    return (
        <>
            <ProjectsSection agentId={agentId} />
            <SessionList
                groups={groups}
                onNew={newConversation}
                onDelete={(key) => {
                    const c = items.find((i) => `${i.channel}#${i.latest_topic}` === key)
                    if (c) setDeleting({ topic: c.latest_topic, title: conversationTitle(c.title, c.latest_topic) })
                }}
                onRename={async (key, title) => {
                    const c = items.find((i) => `${i.channel}#${i.latest_topic}` === key)
                    if (!c) return
                    try {
                        await renameTopic(agentId, c.latest_topic, title)
                        setListVersion((v) => v + 1)
                    } catch (e) {
                        toast.error("Couldn't rename the conversation", e instanceof Error ? e.message : undefined)
                    }
                }}
                search={{ value: sessionQuery, onChange: setSessionQuery }}
                empty={sessionSearch ? 'No conversation matched.' : undefined}
                showAll={
                    total > items.length
                        ? { label: 'Show more', onSelect: () => setShown((n) => n + SESSIONS_STEP) }
                        : undefined
                }
            />
            <DeleteConversation
                agentId={agentId}
                target={deleting}
                onClose={() => setDeleting(null)}
                onDeleted={(topic) => {
                    setListVersion((v) => v + 1)
                    if (topicId && decodeURIComponent(topicId) === topic) navigate(`/${agentId}/chat/${nanoid(10)}`)
                }}
            />
        </>
    )
}
