import { Icon, IconButton, Notice } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import { renameTopic } from '../../api/endpoints/chat'
import {
    createProjectSession,
    getGitSummary,
    projectChannel,
    type Project,
    type ProjectSessionRow,
} from '../../api/endpoints/projects'
import { sidebarTime } from '../../lib/conversations'
import { PROJECT_SESSION_PREVIEW, projectSessionRows } from '../../lib/projectSessions'
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'
import { DeleteConversation, type DeleteTarget } from '../chat/DeleteConversation'
import { DeleteWorktreeDialog } from './DeleteWorktreeDialog'
import { RemoveProjectDialog } from './RemoveProjectDialog'
import { WorktreeDialog } from './WorktreeDialog'

/** The last component of a folder path, for the sidebar's one-line folder hint. */
export function folderName(folder: string) {
    const parts = folder.split(/[\\/]/).filter(Boolean)
    return parts[parts.length - 1] ?? folder
}

type Props = {
    agentId: string
    project: Project
    sessions: ProjectSessionRow[]
}

/**
 * One project above Sessions (FR-005): its folder name and availability, then the caller's
 * sessions always listed — the same rows as the Sessions groups, worktree chats at the top, five
 * of the rest until Show all — each with rename and delete on hover.
 */
export function ProjectRow({ agentId, project, sessions }: Props) {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const activeTopic = pathname.startsWith(`/${agentId}/projects/${project.id}/chat/`)
        ? decodeURIComponent(pathname.split('/').pop() ?? '')
        : ''
    const insertSession = useProjects((s) => s.insertSession)
    const loadSessions = useProjects((s) => s.loadSessions)
    const refresh = useProjects((s) => s.refresh)
    const [busy, setBusy] = useState(false)
    const [query, setQuery] = useState('')
    const [searching, setSearching] = useState(false)
    // Five sessions is a preview, not a limit: the rest are one click away.
    const [showAll, setShowAll] = useState(false)
    const [removing, setRemoving] = useState(false)
    const folder = folderName(project.folder)
    // Like the Sessions list: the field opens from the header and stays open while it holds text.
    const searchOpen = searching || !!query
    const closeSearch = () => {
        setQuery('')
        setSearching(false)
    }
    const [renamingTopic, setRenamingTopic] = useState<string | null>(null)
    const [draft, setDraft] = useState('')
    const [deleting, setDeleting] = useState<DeleteTarget | null>(null)
    const [creatingWorktree, setCreatingWorktree] = useState(false)
    // A worktree needs a git repository: ask once when the project is opened in the sidebar.
    const [isRepo, setIsRepo] = useState(false)
    useEffect(() => {
        if (!project.available) return
        let alive = true
        getGitSummary(agentId, project.id)
            .then((summary) => alive && setIsRepo(!!summary?.available))
            .catch(() => alive && setIsRepo(false))
        return () => {
            alive = false
        }
    }, [agentId, project.id, project.available])

    const matched = query.trim()
        ? sessions.filter((s) =>
              (s.title || 'New session').toLowerCase().includes(query.trim().toLowerCase())
          )
        : sessions
    // A search is a deliberate narrowing of the list, so what it matched is shown whole.
    const searchingYet = !!query.trim()
    const { rows, total } = projectSessionRows(matched, showAll || searchingYet)
    // The toggle outlives the click that expands the list; while searching there is nothing to toggle.
    const canToggle = !searchingYet && total > PROJECT_SESSION_PREVIEW

    const start = async () => {
        setBusy(true)
        try {
            const session = await createProjectSession(agentId, project.id)
            insertSession(agentId, project.id, {
                topic: session.topic,
                title: session.title,
                is_thinking: false,
                last_activity: null,
            })
            navigate(`/${agentId}/projects/${project.id}/chat/${session.topic}`)
        } catch (e) {
            toast.error("Couldn't start a session", e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    const rename = async (topic: string) => {
        const title = draft.trim()
        if (title.length < 1 || title.length > 120) {
            toast.error('The name must be 1–120 characters')
            return
        }
        setBusy(true)
        try {
            await renameTopic(agentId, topic, title, projectChannel(project.id))
            setRenamingTopic(null)
            await loadSessions(agentId, project.id)
        } catch (e) {
            toast.error("Couldn't rename the session", e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <div className="flex flex-col">
            <div className="group/project flex items-center gap-4 px-10 pt-8 pb-4">
                <span className="flex min-w-0 flex-1 items-center gap-6" title={project.folder}>
                    <span className="truncate type-small text-fg">{folder}</span>
                    {!project.available && (
                        <Icon name="alert" size={12} tone="failed" aria-label="Folder unavailable" />
                    )}
                </span>
                <span className="flex shrink-0 items-center opacity-0 transition-opacity group-hover/project:opacity-100 group-focus-within/project:opacity-100">
                    <IconButton
                        size="sm"
                        variant="ghost"
                        icon="trash"
                        className="enabled:hover:text-failed! enabled:active:text-failed!"
                        aria-label={`Remove ${folder}`}
                        title="Remove project"
                        onClick={() => setRemoving(true)}
                    />
                </span>
                {sessions.length > 0 && (
                    <IconButton
                        size="sm"
                        variant="ghost"
                        icon="search"
                        aria-label="Search sessions"
                        title="Search sessions"
                        aria-pressed={searchOpen}
                        onClick={() => (searchOpen ? closeSearch() : setSearching(true))}
                    />
                )}
                {isRepo && (
                    <IconButton
                        size="sm"
                        variant="ghost"
                        icon="git-fork"
                        aria-label={`New worktree in ${folder}`}
                        title="New worktree"
                        disabled={busy}
                        onClick={() => setCreatingWorktree(true)}
                    />
                )}
                <IconButton
                    size="sm"
                    variant="ghost"
                    icon="plus"
                    aria-label={`New session in ${folder}`}
                    disabled={busy}
                    onClick={() => void start()}
                />
            </div>

            {!project.available && (
                <div className="px-10 pb-4">
                    <Notice tone="failed" compact icon="alert">
                        The folder is unavailable; history is readable, commands are refused.
                    </Notice>
                </div>
            )}
            {searchOpen && (
                <label className="mx-10 mb-4 flex h-30 items-center gap-8 rounded-lg border border-border bg-surface px-10 focus-within:border-border-hover">
                    <Icon name="search" size={13} tone="faint" />
                    <input
                        autoFocus={searching}
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        onKeyDown={(e) => e.key === 'Escape' && closeSearch()}
                        placeholder="Search sessions"
                        aria-label={`Search sessions in ${folder}`}
                        className="min-w-0 flex-1 bg-transparent type-small text-fg outline-none placeholder:text-faint"
                    />
                </label>
            )}
            {sessions.length === 0 ? (
                <p className="px-10 type-small text-faint">No sessions yet. New session starts one.</p>
            ) : (
                <>
                    {rows.length === 0 && (
                        <p className="px-10 type-small text-faint">No session matched.</p>
                    )}
                    <ul aria-label={`${folder} sessions`} className="m-0 flex list-none flex-col p-0">
                        {rows.map((session) => {
                            const title = session.title || 'New session'
                            const href = `/${agentId}/projects/${project.id}/chat/${session.topic}`
                            const active = activeTopic === session.topic
                            if (renamingTopic === session.topic) {
                                return (
                                    <li key={session.topic}>
                                        <input
                                            autoFocus
                                            aria-label="Session name"
                                            value={draft}
                                            disabled={busy}
                                            maxLength={120}
                                            onChange={(e) => setDraft(e.target.value)}
                                            onBlur={() => !busy && setRenamingTopic(null)}
                                            onKeyDown={(e) => {
                                                if (e.key === 'Enter') void rename(session.topic)
                                                if (e.key === 'Escape') setRenamingTopic(null)
                                            }}
                                            className="h-30 w-full rounded-lg border border-border-hover bg-surface px-10 type-ui text-fg outline-none"
                                        />
                                    </li>
                                )
                            }
                            return (
                                <li key={session.topic} className="group/row relative">
                                    <button
                                        type="button"
                                        onClick={() => navigate(href)}
                                        aria-current={active ? 'page' : undefined}
                                        className={`relative flex h-30 w-full items-center gap-8 rounded-lg px-10 text-left transition-colors ${
                                            active
                                                ? 'bg-surface text-fg'
                                                : 'text-muted hover:bg-surface/60 hover:text-soft'
                                        }`}
                                    >
                                        {session.is_thinking && (
                                            <span
                                                role="img"
                                                aria-label="a turn is running"
                                                className="size-6 shrink-0 rounded-pill bg-live"
                                            />
                                        )}
                                        {session.worktree && (
                                            <span
                                                className="shrink-0"
                                                title={
                                                    session.worktree.available
                                                        ? `Worktree on ${session.worktree.branch}`
                                                        : 'Worktree folder unavailable'
                                                }
                                            >
                                                <Icon
                                                    name="git-fork"
                                                    size={12}
                                                    tone={session.worktree.available ? 'faint' : 'failed'}
                                                    aria-label={
                                                        session.worktree.available
                                                            ? 'Worktree'
                                                            : 'Worktree unavailable'
                                                    }
                                                />
                                            </span>
                                        )}
                                        <span
                                            className={`min-w-0 flex-1 truncate type-ui ${active ? 'type-ui-medium' : ''}`}
                                        >
                                            {title}
                                        </span>
                                        {session.worktree && session.worktree.branch !== title && (
                                            <span className="shrink-0 truncate type-mono-xs text-faint">
                                                {session.worktree.branch}
                                            </span>
                                        )}
                                        {session.last_activity != null && (
                                            <span
                                                className={`shrink-0 type-mono-xs transition-opacity ${
                                                    session.is_thinking
                                                        ? 'text-live-text'
                                                        : 'text-faint'
                                                } group-hover/row:opacity-0 group-focus-within/row:opacity-0`}
                                            >
                                                {sidebarTime(session.last_activity)}
                                            </span>
                                        )}
                                    </button>
                                    <button
                                        type="button"
                                        aria-label={`Rename ${title}`}
                                        title="Rename session"
                                        onClick={() => {
                                            setDraft(session.title || '')
                                            setRenamingTopic(session.topic)
                                        }}
                                        className="absolute top-1/2 right-30 grid size-24 -translate-y-1/2 place-items-center rounded-md text-faint opacity-0 transition-[opacity,color,background-color] hover:bg-raised hover:text-fg focus-visible:opacity-100 group-hover/row:opacity-100 group-focus-within/row:opacity-100"
                                    >
                                        <Icon name="pencil" size={13} />
                                    </button>
                                    <button
                                        type="button"
                                        aria-label={`Delete ${title}`}
                                        title="Delete conversation"
                                        onClick={() => setDeleting({ topic: session.topic, title })}
                                        className="absolute top-1/2 right-6 grid size-24 -translate-y-1/2 place-items-center rounded-md text-faint opacity-0 transition-[opacity,color,background-color] hover:bg-raised hover:text-failed focus-visible:opacity-100 group-hover/row:opacity-100 group-focus-within/row:opacity-100"
                                    >
                                        <Icon name="trash" size={13} />
                                    </button>
                                </li>
                            )
                        })}
                    </ul>
                    {canToggle && (
                        <button
                            type="button"
                            onClick={() => setShowAll((all) => !all)}
                            aria-expanded={showAll}
                            className="px-10 py-8 text-left type-small text-faint transition-colors hover:text-fg"
                        >
                            {showAll ? 'Show less' : 'Show all'}
                        </button>
                    )}
                </>
            )}

            <RemoveProjectDialog
                agentId={agentId}
                project={removing ? { id: project.id, name: folder } : null}
                onClose={() => setRemoving(false)}
            />
            <WorktreeDialog
                agentId={agentId}
                projectId={project.id}
                open={creatingWorktree}
                onClose={() => setCreatingWorktree(false)}
            />
            {deleting && sessions.find((s) => s.topic === deleting.topic)?.worktree ? (
                <DeleteWorktreeDialog
                    agentId={agentId}
                    projectId={project.id}
                    worktree={{
                        id: sessions.find((s) => s.topic === deleting.topic)!.worktree!.id,
                        name: sessions.find((s) => s.topic === deleting.topic)!.worktree!.name,
                        topic: deleting.topic,
                    }}
                    onClose={() => setDeleting(null)}
                    onDeleted={(topic) => {
                        setDeleting(null)
                        void loadSessions(agentId, project.id)
                        void refresh(agentId)
                        if (activeTopic === topic) navigate(`/${agentId}/chat`)
                    }}
                />
            ) : (
                <DeleteConversation
                    agentId={agentId}
                    channel={projectChannel(project.id)}
                    target={deleting}
                    onClose={() => setDeleting(null)}
                    onDeleted={(topic) => {
                        void loadSessions(agentId, project.id)
                        void refresh(agentId)
                        if (activeTopic === topic) navigate(`/${agentId}/chat`)
                    }}
                />
            )}
        </div>
    )
}
