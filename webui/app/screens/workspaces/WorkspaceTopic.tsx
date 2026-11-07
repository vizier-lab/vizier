import { EmptyState, JumpToLatest, LoadMore, Notice, Presence } from '@vizier/design'
import { motion } from 'motion/react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'
import { getWorkspace, markTopicRead, reactTo, stopAll, type WorkspaceDetail } from '../../api/endpoints/workspaces'
import { ApiError } from '../../api/client'
import { emptyTopicView } from '../../lib/workspaceFrames'
import { useSession } from '../../stores/session'
import { toast } from '../../stores/toasts'
import { topicKey, useWorkspaces } from '../../stores/workspaces'
import { MembersDialog } from './MembersDialog'
import { WorkspaceHeader } from './WorkspaceHeader'
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
    const [away, setAway] = useState(false)
    const [unseen, setUnseen] = useState(0)
    const lastCount = useRef(view.entries.length)
    useEffect(() => {
        const el = scroller.current
        if (el && pinned.current) {
            el.scrollTop = el.scrollHeight
            setUnseen(0)
        } else if (view.entries.length > lastCount.current) {
            setUnseen((n) => n + (view.entries.length - lastCount.current))
        }
        lastCount.current = view.entries.length
    }, [view.entries, view.steps, view.working])
    // The composer floats over the list, as in a direct chat: the list keeps room for it.
    const composerBox = useRef<HTMLDivElement>(null)
    const [composerHeight, setComposerHeight] = useState(120)
    useEffect(() => {
        const el = composerBox.current
        if (!el) return
        const ro = new ResizeObserver(() => setComposerHeight(el.offsetHeight))
        ro.observe(el)
        return () => ro.disconnect()
    }, [])

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
            <WorkspaceHeader
                detail={detail}
                workspaceId={workspaceId}
                topicId={topicId}
                working={working.length}
                stopping={stopping}
                onStop={() => void stop()}
                onMembers={() => setParams({ members: '1' })}
            />

            {view.connection === 'offline' && (
                <div className="px-24">
                    <Notice tone="failed" compact>
                        Offline. <button className="underline" onClick={() => retry(workspaceId, topicId)}>Retry now</button>
                    </Notice>
                </div>
            )}

            <div className="relative flex min-h-0 flex-1 flex-col">
                <div
                    ref={scroller}
                    className="flex min-h-0 flex-1 flex-col overflow-y-auto px-24 pt-24"
                    onScroll={(e) => {
                        const el = e.currentTarget
                        const near = el.scrollHeight - el.scrollTop - el.clientHeight < 48
                        pinned.current = near
                        setAway(!near)
                        if (near) setUnseen(0)
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
                            <WorkspaceTimeline
                                entries={view.entries}
                                steps={view.steps}
                                me={me}
                                names={names}
                                working={view.working}
                                onReact={(uid, emoji, action) => {
                                    void reactTo(workspaceId, topicId, uid, emoji, action).catch((e) =>
                                        toast.error(e instanceof ApiError ? e.message : "Couldn't save the reaction")
                                    )
                                }}
                            />
                        )}
                    </div>
                    {/* Room for the composer floating over the list. A spacer, not padding-bottom: WebKit
                        leaves a flex scroll container's end padding out of what can be scrolled to. */}
                    <div aria-hidden className="shrink-0 transition-[height] duration-[var(--vz-duration-slow)]" style={{ height: composerHeight + 16 }} />
                </div>
                <Presence>
                    {away && (
                        <motion.div
                            key="jump"
                            className="absolute inset-x-0 flex justify-center"
                            style={{ bottom: composerHeight + 8 }}
                            initial={{ opacity: 0, y: 8 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: 8 }}
                        >
                            <JumpToLatest
                                count={unseen}
                                onClick={() => {
                                    const el = scroller.current
                                    if (el) el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' })
                                    pinned.current = true
                                    setAway(false)
                                    setUnseen(0)
                                }}
                            />
                        </motion.div>
                    )}
                </Presence>
                {/* The composer floats over the messages, over a fade to the page colour, as in a direct chat. */}
                <div ref={composerBox} className="pointer-events-none absolute inset-x-0 bottom-0 z-10">
                    <div aria-hidden className="absolute inset-0 bg-gradient-to-t from-panel via-panel/80 to-transparent" />
                    <div className="pointer-events-auto relative mx-auto w-full max-w-4xl px-24 pt-24 pb-16">
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
                            onPost={async (text, files) => {
                                await post(workspaceId, topicId, text, files)
                                pinned.current = true
                                setAway(false)
                            }}
                        />
                    </div>
                </div>
            </div>
            <MembersDialog
                open={membersOpen}
                workspaceId={workspaceId}
                members={detail?.members ?? []}
                canManage={detail?.role === 'owner' || detail?.role === 'admin'}
                onClose={closeMembers}
                onChanged={() => {
                    void load()
                    getWorkspace(workspaceId).then(setDetail).catch(() => {})
                }}
            />
        </div>
    )
}
