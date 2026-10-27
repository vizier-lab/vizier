import {
    Button,
    EmptyState,
    FilterTabs,
    LoadMore,
    Skeleton,
    SlideOver,
    TextLink,
} from '@vizier/design'
import { useNotifications } from '../../stores/notifications'
import { useAgents } from '../../stores/agents'
import { NotificationRow } from './NotificationRow'
import { SetupActivity } from './SetupActivity'

const EMPTY_STATES = {
    all: {
        icon: 'bell' as const,
        title: 'Nothing yet',
        message:
            'Task runs, dreams, background jobs and integration changes will show up here.',
    },
    unread: {
        icon: 'circle-check' as const,
        title: 'You’re all caught up.',
        message: 'New notifications will appear here.',
    },
    read: {
        icon: 'history' as const,
        title: 'Nothing read yet.',
        message: 'Notifications you open or mark read will collect here.',
    },
}

/**
 * The notification inbox: a right-hand slide-over (full-screen on a phone)
 * over the current screen, so opening it never loses the person's place.
 * Live setup sits above the filters and is deliberately not a read/unread row.
 */
export function NotificationsPanel() {
    const open = useNotifications((s) => s.open)
    const filter = useNotifications((s) => s.filter)
    const items = useNotifications((s) => s.items)
    const counts = useNotifications((s) => s.counts)
    const next = useNotifications((s) => s.next)
    const loading = useNotifications((s) => s.loading)
    const loadingMore = useNotifications((s) => s.loadingMore)
    const loaded = useNotifications((s) => s.loaded)
    const error = useNotifications((s) => s.error)
    const closePanel = useNotifications((s) => s.closePanel)
    const setFilter = useNotifications((s) => s.setFilter)
    const load = useNotifications((s) => s.load)
    const loadMore = useNotifications((s) => s.loadMore)
    const markAll = useNotifications((s) => s.markAll)
    const setups = useAgents((s) => s.setups)

    const active = Object.values(setups)
        .filter((s) => s.phase === 'starting' || s.phase === 'downloading')
        .sort((a, b) => a.agent_id.localeCompare(b.agent_id))
    const empty = EMPTY_STATES[filter]

    return (
        <SlideOver
            open={open}
            title="Notifications"
            onClose={closePanel}
            className="pb-[env(safe-area-inset-bottom)]"
        >
            {active.length > 0 && (
                <section aria-label="Setting up" className="flex flex-col gap-12">
                    {active.map((setup) => (
                        <SetupActivity key={setup.agent_id} setup={setup} />
                    ))}
                </section>
            )}

            {/* Sticky, so the list scrolls under it and the filter stays in reach. */}
            <div className="sticky top-0 z-10 -mx-20 flex flex-wrap items-center justify-between gap-8 border-b border-border bg-surface px-20 py-8">
                <FilterTabs
                    aria-label="Filter notifications"
                    value={filter}
                    onChange={setFilter}
                    options={[
                        { value: 'unread', label: 'Unread', count: counts.unread },
                        { value: 'read', label: 'Read', count: counts.read },
                        { value: 'all', label: 'All', count: counts.all },
                    ]}
                />
                <TextLink
                    onClick={() => void markAll()}
                    disabled={counts.unread === 0}
                >
                    Mark all read
                </TextLink>
            </div>

            {error && items.length === 0 ? (
                <EmptyState
                    kind="error"
                    title="Couldn’t load notifications"
                    message={error}
                    action={
                        <Button size="sm" variant="secondary" onClick={() => void load()}>
                            Retry
                        </Button>
                    }
                />
            ) : loading && !loaded ? (
                <div className="flex flex-col gap-12" aria-busy="true">
                    <Skeleton className="h-44" />
                    <Skeleton className="h-44" />
                    <Skeleton className="h-44" />
                </div>
            ) : items.length === 0 ? (
                <EmptyState
                    icon={empty.icon}
                    title={empty.title}
                    message={empty.message}
                />
            ) : (
                <div className="flex flex-col gap-8">
                    {items.map((item) => (
                        <NotificationRow
                            key={item.id}
                            item={item}
                            onNavigate={closePanel}
                        />
                    ))}
                </div>
            )}

            {next && items.length > 0 && (
                <LoadMore
                    label="Load older"
                    loading={loadingMore}
                    onLoad={() => void loadMore()}
                />
            )}
        </SlideOver>
    )
}
