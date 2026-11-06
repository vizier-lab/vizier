import { Button, EmptyState, LoadMore, Notice } from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'
import { getWorkspace, markTopicRead, stopAll, type WorkspaceDetail } from '../../api/endpoints/workspaces'
import { ApiError } from '../../api/client'
import { emptyTopicView } from '../../lib/workspaceFrames'
import { useSession } from '../../stores/session'
import { toast } from '../../stores/toasts'
import { topicKey, useWorkspaces } from '../../stores/workspaces'
import { MembersDialog } from './MembersDialog'
import { WorkspaceComposer } from './WorkspaceComposer'
import { WorkspaceTimeline } from './WorkspaceTimeline'

/** A Workspace topic: the conversation, who is working, and where to write. */
export function WorkspaceTopic({ workspaceId, topicId }: { workspaceId: string; topicId: string }) {
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const me = useSession((s) => s.user?.username ?? '')
    const { open, loadOlder, post, retry, load } = useWorkspaces()
    const view = useWorkspaces((s) => s.topics[topicKey(workspaceId, topicId)]) ?? emptyTopicView()
    const [detail, setDetail] = useState<WorkspaceDetail | null>(null)
    const [stopping, setStopping] = useState(false)

    // Opening a topic reads it: its notifications, and the unread count on the rail.
    useEffect(() => {
        void markTopicRead(workspaceId, topicId)
            .then((r) => (r.marked > 0 ? load() : undefined))
            .catch(() => {})
    }, [workspaceId, topicId, load])
    // The socket lives as long as the topic is on screen.
    useEffect(() => open(workspaceId, topicId), [open, workspaceId, topicId])
    // Members (and so names and the @ menu) are read again whenever they change.
    useEffect(() => {
        let live = true
        getWorkspace(workspaceId)
            .then((d) => live && setDetail(d))
            .catch(() => live && setDetail(null))
        return () => {
            live = false
        }
    }, [workspaceId, view.closed, view.entries.length === 0, view.membersStale])
    // Removed or deleted: nothing more to see here.
    useEffect(() => {
        if (view.closed === 'removed' || view.closed === 'deleted') {
            void load()
            navigate('/', { replace: true })
        }
    }, [view.closed, load, navigate])

    const names = useMemo(() => Object.fromEntries((detail?.members ?? []).map((m) => [m.handle, m.name])), [detail])
    const topic = detail?.topics.find((t) => t.id === topicId)
    const working = Object.keys(view.working)

    // Stay at the bottom while it grows, unless the person scrolled up.
    const scroller = useRef<HTMLDivElement>(null)
    const pinned = useRef(true)
    useEffect(() => {
        const el = scroller.current
        if (el && pinned.current) el.scrollTop = el.scrollHeight
    }, [view.entries, view.steps, view.working])

    const stop = useCallback(async () => {
        setStopping(true)
        try {
            await stopAll(workspaceId)
        } catch (e) {
            toast.error(e instanceof ApiError ? e.message : "Couldn't stop the agents")
        } finally {
            setStopping(false)
        }
    }, [workspaceId])

    const membersOpen = params.has('members')
    const closeMembers = () => {
        const next = new URLSearchParams(params)
        next.delete('members')
        setParams(next, { replace: true })
    }

    return (
        <div className="flex min-h-0 flex-1 flex-col">
            <header className="flex shrink-0 items-center gap-12 px-24 py-12">
                <div className="flex min-w-0 flex-col">
                    <h1 className="truncate type-heading text-fg">#{topic?.name ?? '…'}</h1>
                    <span className="truncate type-mono-xs text-faint">
                        {detail?.name ?? ''} · {detail?.members.filter((m) => m.kind === 'person').length ?? 0} people ·{' '}
                        {detail?.members.filter((m) => m.kind === 'agent').length ?? 0} agents
                    </span>
                </div>
                <div className="ml-auto flex items-center gap-8">
                    {working.length > 0 && (
                        <Button icon="square" loading={stopping} onClick={() => void stop()}>
                            Stop all
                        </Button>
                    )}
                    <Button icon="users" onClick={() => setParams({ members: '1' })}>
                        Members
                    </Button>
                </div>
            </header>

            {view.connection === 'offline' && (
                <div className="px-24">
                    <Notice tone="failed" compact>
                        Offline. <button className="underline" onClick={() => retry(workspaceId, topicId)}>Retry now</button>
                    </Notice>
                </div>
            )}

            <div
                ref={scroller}
                className="flex min-h-0 flex-1 flex-col overflow-y-auto px-24 pt-12"
                onScroll={(e) => {
                    const el = e.currentTarget
                    pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48
                }}
            >
                <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-20">
                    {view.nextBefore != null && (
                        <LoadMore look="auto" label="Load older" loading={false} onLoad={() => void loadOlder(workspaceId, topicId)} />
                    )}
                    {view.loaded && view.entries.length === 0 && view.steps.length === 0 ? (
                        <div className="my-auto">
                            <EmptyState
                                title="Nothing here yet"
                                message="Say something. Agents read along and answer only when you @mention them."
                            />
                        </div>
                    ) : (
                        <WorkspaceTimeline entries={view.entries} steps={view.steps} me={me} names={names} working={view.working} />
                    )}
                </div>
                <div aria-hidden className="h-16 shrink-0" />
            </div>

            <div className="shrink-0 px-24 pb-16">
                <div className="mx-auto w-full max-w-4xl">
                    {view.postError && (
                        <Notice tone="failed" compact>
                            {view.postError}
                        </Notice>
                    )}
                    <WorkspaceComposer
                        workspaceId={workspaceId}
                        topicName={topic?.name ?? 'topic'}
                        me={me}
                        members={detail?.members ?? []}
                        onPost={async (text) => {
                            await post(workspaceId, topicId, text)
                            pinned.current = true
                        }}
                    />
                </div>
            </div>
            <MembersDialog open={membersOpen} members={detail?.members ?? []} onClose={closeMembers} />
        </div>
    )
}
