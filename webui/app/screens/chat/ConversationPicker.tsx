import {
    ConversationRow,
    Picker,
    PickerAction,
    Segment,
    type PickerProps,
} from '@vizier/design'
import {
    useCallback,
    useEffect,
    useMemo,
    useRef,
    useState,
    type ReactElement,
} from 'react'
import {
    listConversations,
    type ConversationItem,
    type ConversationKind,
    type ConversationPage,
} from '../../api/endpoints/conversations'
import { count } from '../../lib/format'
import { conversationTitle, rowTime, sections } from '../../lib/conversations'

type Props = {
    agentId: string
    /** The button that opens the picker (the TopicHeader). */
    /** Absent: the list shows inline, always open (the phone layout). */
    trigger?: PickerProps<ConversationItem>['trigger'] & ReactElement
    width?: number
    open: boolean
    onOpenChange: (open: boolean) => void
    onSelect: (conversation: ConversationItem) => void
    onNew: () => void
    /** The conversation open now: `channel` and, for the WebUI, its topic. */
    current?: { topic?: string; channel?: string }
    /** Topics with a turn running, to show a live dot. */
    liveTopics?: string[]
}

const KINDS: { value: ConversationKind | 'all'; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'webui', label: 'WebUI' },
    { value: 'discord', label: 'Discord' },
    { value: 'telegram', label: 'Telegram' },
]

const SEARCH_MS = 250

/**
 * Every conversation with an agent across the WebUI, Discord and Telegram, newest first. Each
 * change of search or filter fetches a page from the server; scrolling to the end fetches the
 * next, so a thousand conversations cost one page at a time.
 */
export function ConversationPicker({
    agentId,
    trigger,
    width = 400,
    open,
    onOpenChange,
    onSelect,
    onNew,
    current,
    liveTopics = [],
}: Props) {
    const [query, setQuery] = useState('')
    const [kind, setKind] = useState<ConversationKind | 'all'>('all')
    const [items, setItems] = useState<ConversationItem[]>([])
    const [counts, setCounts] = useState<ConversationPage['counts']>()
    const [total, setTotal] = useState(0)
    const [next, setNext] = useState<string | null>(null)
    // Loading from the start: the first fetch waits a tick after opening, and an empty list
    // shown before it would read as "no conversations".
    const [loading, setLoading] = useState(true)
    const [failed, setFailed] = useState(false)
    const generation = useRef(0)

    const fetchPage = useCallback(
        async (cursor?: string) => {
            const mine = ++generation.current
            setLoading(true)
            try {
                const page = await listConversations(agentId, {
                    q: query.trim(),
                    channel: kind,
                    cursor,
                })
                if (mine !== generation.current) return
                setItems((prev) =>
                    cursor ? [...prev, ...page.items] : page.items
                )
                setCounts(page.counts)
                setTotal(page.total)
                setNext(page.next ?? null)
                setFailed(false)
            } catch {
                if (mine === generation.current) setFailed(true)
            } finally {
                if (mine === generation.current) setLoading(false)
            }
        },
        [agentId, query, kind]
    )

    // A new search or filter starts over, after a pause for typing; opening it looks again.
    useEffect(() => {
        if (!open) return
        const id = setTimeout(() => void fetchPage(), query ? SEARCH_MS : 0)
        return () => clearTimeout(id)
    }, [open, fetchPage]) // eslint-disable-line react-hooks/exhaustive-deps

    const groups = useMemo(() => sections(items), [items])
    const remaining = Math.max(0, total - items.length)
    const key = (c: ConversationItem) => `${c.channel}#${c.latest_topic}`
    const isCurrent = (c: ConversationItem) =>
        current?.channel
            ? c.channel === current.channel
            : c.kind === 'webui' && c.latest_topic === current?.topic

    return (
        <Picker<ConversationItem>
            label="Conversations"
            width={width}
            itemHeight={52}
            open={open}
            onOpenChange={onOpenChange}
            trigger={trigger}
            search={{
                placeholder: 'Search conversations',
                total: counts ? count(counts.all) : undefined,
                value: query,
                onChange: setQuery,
                mode: 'server',
            }}
            filters={
                <>
                    {KINDS.map((k) => (
                        <Segment
                            key={k.value}
                            selected={kind === k.value}
                            count={counts ? count(counts[k.value]) : undefined}
                            onClick={() => setKind(k.value)}
                        >
                            {k.label}
                        </Segment>
                    ))}
                </>
            }
            sections={groups}
            getKey={key}
            loading={loading}
            onEndReached={next ? () => void fetchPage(next) : undefined}
            renderItem={(c, { query: q }) => (
                <ConversationRow
                    kind={c.kind}
                    title={conversationTitle(c.title, c.latest_topic)}
                    query={q}
                    place={c.place}
                    live={
                        c.kind === 'webui' &&
                        liveTopics.includes(c.latest_topic)
                    }
                    time={rowTime(c.last_activity)}
                    selected={isCurrent(c)}
                />
            )}
            onSelect={onSelect}
            empty={
                failed ? (
                    <>
                        <span className="type-body-strong text-fg">Couldn’t load conversations</span>
                        <span>Close and reopen to try again.</span>
                    </>
                ) : query.trim() || kind !== 'all' ? (
                    <>
                        <span className="type-body-strong text-fg">No conversation matched</span>
                        <span>Try another search or filter.</span>
                    </>
                ) : (
                    <>
                        <span className="type-body-strong text-fg">No conversations yet</span>
                        <span>Start one below.</span>
                    </>
                )
            }
            remainder={
                failed && items.length > 0
                    ? 'Couldn’t load more conversations'
                    : remaining > 0
                      ? `${count(remaining)} more · scroll to load`
                      : undefined
            }
            footer={
                <PickerAction icon="plus" onClick={onNew}>
                    New conversation
                </PickerAction>
            }
        />
    )
}
