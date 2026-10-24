import { Notice } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useParams, useSearchParams } from 'react-router'
import { topicHistory } from '../api/endpoints/chat'
import { listConversations, type ConversationItem } from '../api/endpoints/conversations'
import { emptyTopic, reduce, type TopicState } from '../lib/frames'
import { ConversationHeader } from '../screens/chat/ConversationHeader'
import { MessageList } from '../screens/chat/MessageList'
import { useAgents } from '../stores/agents'

const PAGE = 50

/** "Discord conversation 123" until the picker has said more. */
function labelOf(channel: string): { platform: 'Discord' | 'Telegram'; title: string } {
    const platform = channel.startsWith('telegram__') ? 'Telegram' : 'Discord'
    const id = channel.split('__')[1] ?? channel
    return { platform, title: `${platform === 'Telegram' ? 'Telegram chat' : 'Discord channel'} ${id}` }
}

/**
 * A Discord or Telegram conversation, opened from the picker: its history, with no composer.
 * The agent answers there, on the platform; this page only lets you read it.
 */
export default function ChatRead() {
    const { agentId = '' } = useParams()
    const [params] = useSearchParams()
    const channel = params.get('channel') ?? ''
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const agentName = agent?.name ?? agentId
    const { platform, title: fallbackTitle } = labelOf(channel)
    const [found, setFound] = useState<ConversationItem>()

    // The picker's list is where the server and channel names are resolved, so ask it.
    useEffect(() => {
        let live = true
        setFound(undefined)
        void (async () => {
            try {
                let cursor: string | undefined
                for (let page = 0; page < 20 && live; page++) {
                    const res = await listConversations(agentId, { channel: platform.toLowerCase() as 'discord' | 'telegram', cursor, limit: 100 })
                    const hit = res?.items.find((i) => i.channel === channel)
                    if (hit) return live && setFound(hit)
                    if (!res?.next) return
                    cursor = res.next
                }
            } catch {
                // keep the id-only label
            }
        })()
        return () => {
            live = false
        }
    }, [agentId, channel, platform])

    const id = channel.split('__')[1] ?? channel
    const named = found && found.title !== fallbackTitle
    const title = found?.title ?? fallbackTitle
    const meta = [platform.toLowerCase(), named && found.place !== platform ? found.place : undefined, `${platform === 'Discord' ? 'channel' : 'chat'} ${id}`, 'read-only']
        .filter(Boolean)
        .join(' · ')

    const [state, setState] = useState<TopicState>(() => emptyTopic())
    const [exhausted, setExhausted] = useState(false)
    const [loadingOlder, setLoadingOlder] = useState(false)
    const [error, setError] = useState<string>()

    useEffect(() => {
        let live = true
        setState(emptyTopic())
        setExhausted(false)
        setError(undefined)
        topicHistory(agentId, 'DEFAULT', { channel, limit: PAGE }).then(
            (entries) => {
                if (!live) return
                setState((s) => reduce(s, { type: 'history', entries: entries ?? [] }))
                setExhausted((entries?.length ?? 0) < PAGE)
            },
            (e) => live && setError(e instanceof Error ? e.message : 'The conversation could not be loaded.')
        )
        return () => {
            live = false
        }
    }, [agentId, channel])

    const loadOlder = useCallback(async () => {
        const oldest = state.entries[0]
        if (!oldest || loadingOlder) return
        setLoadingOlder(true)
        try {
            const older = await topicHistory(agentId, 'DEFAULT', { channel, limit: PAGE, before: oldest.timestamp })
            setState((s) => reduce(s, { type: 'history', entries: older ?? [], position: 'older' }))
            setExhausted((older?.length ?? 0) < PAGE)
        } finally {
            setLoadingOlder(false)
        }
    }, [agentId, channel, state.entries, loadingOlder])

    return (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <ConversationHeader agentId={agentId} title={title} meta={meta} current={{ channel }} />
            <div className="px-24 pt-12">
                <Notice tone="info">
                    This conversation happens on {platform}. You can read it here; to reply, talk to {agentName} there.
                </Notice>
            </div>
            {error ? (
                <div className="p-24">
                    <Notice tone="failed" title="Couldn’t load this conversation">{error}</Notice>
                </div>
            ) : (
                <MessageList
                    state={state}
                    agent={{ id: agentId, name: agentName }}
                    hasOlder={!exhausted}
                    loadingOlder={loadingOlder}
                    onLoadOlder={() => void loadOlder()}
                    onReact={() => {}}
                    byPlatformName
                />
            )}
        </div>
    )
}
