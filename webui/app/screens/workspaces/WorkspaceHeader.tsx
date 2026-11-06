import { Avatar, Button, Icon, Menu, MenuItem, Sigil, useDismiss } from '@vizier/design'
import { useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import type { WorkspaceDetail } from '../../api/endpoints/workspaces'
import { useMobile } from '../../lib/useMobile'
import { NotificationsBell } from '../shared/NotificationsBell'
import { FadeRule } from '../shared/SideCard'

type Props = {
    detail: WorkspaceDetail | null
    workspaceId: string
    topicId: string
    /** Agents with a turn running in this Workspace. */
    working: number
    stopping: boolean
    onStop: () => void
    onMembers: () => void
}

const STACK = 5

/**
 * The top of a topic (the Workspace mockup): `ops / # topic ⌄` that opens the other topics, how
 * many agents are working with Stop all, who is in it as a stack, and the bell.
 */
export function WorkspaceHeader({ detail, workspaceId, topicId, working, stopping, onStop, onMembers }: Props) {
    const navigate = useNavigate()
    const mobile = useMobile()
    const [open, setOpen] = useState(false)
    const box = useRef<HTMLDivElement>(null)
    useDismiss(open, () => setOpen(false), box)

    const topics = detail?.topics ?? []
    const topic = topics.find((t) => t.id === topicId)
    const members = detail?.members ?? []
    const people = members.filter((m) => m.kind === 'person').length
    const agents = members.filter((m) => m.kind === 'agent').length
    const shown = [...members.filter((m) => m.kind === 'agent'), ...members.filter((m) => m.kind === 'person')]
    const extra = Math.max(0, shown.length - STACK)

    return (
        <header className="relative flex h-76 shrink-0 items-center gap-12 px-24 max-md:px-16">
            <FadeRule className="pointer-events-none absolute inset-x-0 bottom-0" />
            <div ref={box} className="relative min-w-0">
                <button
                    type="button"
                    aria-haspopup="menu"
                    aria-expanded={open}
                    onClick={() => setOpen((o) => !o)}
                    className="flex min-w-0 flex-col items-start gap-2 rounded-sm text-left"
                >
                    <span className="flex min-w-0 items-center gap-8">
                        {!mobile && <span className="type-heading text-muted">{detail?.name ?? '…'} /</span>}
                        <span className="truncate type-heading text-fg"># {topic?.name ?? '…'}</span>
                        <Icon name="chevron-down" size={14} tone="muted" />
                    </span>
                    {!mobile && (
                        <span className="type-mono-xs text-faint">
                            topic {Math.max(1, topics.findIndex((t) => t.id === topicId) + 1)} of {topics.length || 1} · {people} people ·{' '}
                            {agents} agents
                        </span>
                    )}
                </button>
                {open && (
                    <div className="absolute top-full left-0 z-30 mt-6">
                        <Menu aria-label="Topics">
                            {topics.map((t) => (
                                <MenuItem
                                    key={t.id}
                                    label={`# ${t.name}`}
                                    selected={t.id === topicId}
                                    meta={t.unread > 0 ? String(t.unread) : undefined}
                                    onClick={() => {
                                        setOpen(false)
                                        navigate(`/workspaces/${workspaceId}/${t.id}`)
                                    }}
                                />
                            ))}
                        </Menu>
                    </div>
                )}
            </div>
            <div className="flex-1" />
            {working > 0 && (
                <>
                    {!mobile && (
                        <span className="flex items-center gap-6 type-mono-xs text-live-text" role="status">
                            <span aria-hidden className="size-6 rounded-full bg-live" />
                            {working} working
                        </span>
                    )}
                    <Button icon="square" size="sm" loading={stopping} onClick={onStop} aria-label="Stop every agent working in this Workspace">
                        {mobile ? undefined : 'Stop all'}
                    </Button>
                </>
            )}
            <button
                type="button"
                onClick={onMembers}
                aria-label={`Members of ${detail?.name ?? 'this Workspace'}`}
                className="flex items-center gap-8 rounded-md py-3 pr-6 pl-4 hover:bg-surface"
            >
                <span className="flex">
                    {shown.slice(0, mobile ? 2 : STACK).map((m, i) => (
                        <span key={`${m.kind}:${m.handle}`} className={i === 0 ? '' : '-ml-5'} style={{ borderRadius: 6, outline: '2px solid var(--vz-panel, transparent)' }}>
                            {m.kind === 'agent' ? <Sigil size="sm" agentId={m.handle} /> : <Avatar name={m.handle} size="sm" square />}
                        </span>
                    ))}
                </span>
                {!mobile && extra > 0 && <span className="type-mono-xs text-faint">+{extra}</span>}
            </button>
            <NotificationsBell />
        </header>
    )
}
