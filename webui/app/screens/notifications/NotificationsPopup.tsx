import {
    Button,
    EmptyState,
    Floating,
    Icon,
    IconButton,
    LoadMore,
    Skeleton,
    useDismiss,
} from '@vizier/design'
import { useEffect, useRef } from 'react'
import { useAgents } from '../../stores/agents'
import type { ViewFilter } from '../../lib/notifications'
import { useNotifications } from '../../stores/notifications'
import { NotificationRow } from './NotificationRow'
import { SetupActivity } from './SetupActivity'

const EMPTY_STATES = {
    all: {
        icon: 'bell' as const,
        title: 'Nothing yet',
        message: 'Task runs, dreams, background work and integration changes will show up here.',
    },
    unread: {
        icon: 'circle-check' as const,
        title: 'You’re all caught up.',
        message: 'New notifications will appear here.',
    },
}

const FILTERS: { value: ViewFilter; label: string; icon: 'bell' | 'list' }[] = [
    { value: 'unread', label: 'Unread', icon: 'bell' },
    { value: 'all', label: 'All', icon: 'list' },
]

/**
 * The notifications popup, hung from the bell (`Floating`, aligned to its right edge). One line of
 * filters with mark-all and a close ×, then live setup, then the list: setup and notifications are
 * the same card shape. It closes on Escape, on a press outside it, and on the ×; a press on the
 * bell is the bell's own toggle, so it is ignored here. With no bell mounted it hangs from a
 * fixed point at the top right.
 */
export function NotificationsPopup() {
    const open = useNotifications((s) => s.open)
    const anchorEl = useNotifications((s) => s.anchor)
    const closePanel = useNotifications((s) => s.closePanel)
    const popup = useRef<HTMLDivElement | null>(null)
    const bell = useRef<HTMLElement | null>(null)
    const fallback = useRef<HTMLDivElement | null>(null)
    bell.current = anchorEl
    useDismiss(open, closePanel, popup, bell)

    // The anchor Floating measures: the bell when there is one, else the fixed top-right point.
    const anchor = useRef<HTMLElement | null>(null)
    anchor.current = anchorEl?.isConnected ? anchorEl : fallback.current

    return (
        <>
            <div
                ref={fallback}
                aria-hidden
                className="pointer-events-none fixed top-12 right-12 size-34"
            />
            {open && <Popup anchor={anchor} popup={popup} />}
        </>
    )
}

function Popup({
    anchor,
    popup,
}: {
    anchor: React.RefObject<HTMLElement | null>
    popup: React.RefObject<HTMLDivElement | null>
}) {
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

    // Focus moves into the popup so Escape and Tab work from the keyboard.
    const heading = useRef<HTMLDivElement | null>(null)
    useEffect(() => heading.current?.focus(), [])

    const active = Object.values(setups)
        .filter((s) => s.phase === 'starting' || s.phase === 'downloading')
        .sort((a, b) => a.agent_id.localeCompare(b.agent_id))
    const empty = EMPTY_STATES[filter]

    return (
        <Floating anchor={anchor} align="end" gap={8} ref={popup} style={{ width: 'min(400px, calc(100vw - 16px))' }}>
            <div
                role="dialog"
                aria-label="Notifications"
                className="flex max-h-[min(560px,calc(100dvh-80px))] flex-col overflow-hidden rounded-2xl border border-border-strong surface-float shadow-popover"
            >
                <div
                    ref={heading}
                    tabIndex={-1}
                    className="flex shrink-0 items-center justify-between gap-8 px-16 pt-14 pb-8 outline-none"
                >
                    <div role="group" aria-label="Filter notifications" className="flex items-center gap-6">
                        {FILTERS.map((f) => (
                            <button
                                key={f.value}
                                type="button"
                                aria-pressed={filter === f.value}
                                onClick={() => filter !== f.value && setFilter(f.value)}
                                className={`inline-flex h-28 items-center gap-6 rounded-pill border px-12 type-small-medium transition-colors ${
                                    filter === f.value
                                        ? 'border-border-strong bg-raised text-fg'
                                        : 'border-transparent text-muted hover:text-fg'
                                }`}
                            >
                                <Icon name={f.icon} size={13} />
                                {f.label}
                                {f.value === 'unread' && counts.unread > 0 && (
                                    <span className="text-faint">{counts.unread}</span>
                                )}
                            </button>
                        ))}
                    </div>
                    <div className="flex items-center gap-4">
                        <IconButton
                            variant="ghost"
                            size="sm"
                            icon="check-check"
                            aria-label="Mark all as read"
                            disabled={counts.unread === 0}
                            onClick={() => void markAll()}
                        />
                        <IconButton
                            variant="ghost"
                            size="sm"
                            icon="close"
                            aria-label="Close notifications"
                            onClick={closePanel}
                        />
                    </div>
                </div>

                <div className="flex min-h-0 flex-1 flex-col gap-8 overflow-y-auto px-12 pt-4 pb-12">
                    {active.length > 0 && (
                        <section aria-label="Setting up" className="flex flex-col gap-8">
                            {active.map((setup) => (
                                <SetupActivity key={setup.agent_id} setup={setup} />
                            ))}
                        </section>
                    )}

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
                        <div className="flex flex-col gap-8" aria-busy="true">
                            <Skeleton className="h-64" />
                            <Skeleton className="h-64" />
                            <Skeleton className="h-64" />
                        </div>
                    ) : items.length === 0 ? (
                        active.length === 0 && (
                            <EmptyState icon={empty.icon} title={empty.title} message={empty.message} />
                        )
                    ) : (
                        <ul className="flex flex-col gap-8" aria-label="Notifications">
                            {items.map((item) => (
                                <li key={item.id}>
                                    <NotificationRow item={item} onNavigate={closePanel} />
                                </li>
                            ))}
                        </ul>
                    )}

                    {next && items.length > 0 && (
                        <LoadMore
                            label="Load older"
                            loading={loadingMore}
                            onLoad={() => void loadMore()}
                        />
                    )}
                </div>
            </div>
        </Floating>
    )
}
