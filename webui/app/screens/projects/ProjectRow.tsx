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
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'
import { DeleteConversation, type DeleteTarget } from '../chat/DeleteConversation'
import { RemoveProjectDialog } from './RemoveProjectDialog'
import { WorktreeDialog } from './WorktreeDialog'
import { DeleteWorktreeDialog } from './DeleteWorktreeDialog'

/** The last component of a folder path, for the sidebar's one-line folder hint. */
export function folderName(folder: string) {
    const parts = folder.split(/[\\/]/).filter(Boolean)
    return parts[parts.length - 1] ?? folder
}

type Props = {
    agentId: string
    project: Project
    sessions: ProjectSessionRow[]
    expanded: boolean
    onToggle: () => void
}

/**
 * One project above Sessions (FR-005): its folder name, availability and session count, and
 * — expanded — the caller's sessions, searchable, each with rename and delete on hover like the
 * main Sessions list.
 */
export function ProjectRow({ agentId, project, sessions, expanded, onToggle }: Props) {
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
        if (!expanded || !project.available) return
        let alive = true
        getGitSummary(agentId, project.id)
            .then((summary) => alive && setIsRepo(!!summary?.available))
            .catch(() => alive && setIsRepo(false))
        return () => {
            alive = false
        }
    }, [expanded, agentId, project.id, project.available])

    const shown = query.trim()
        ? sessions.filter((s) =>
              (s.title || 'New session').toLowerCase().includes(query.trim().toLowerCase())
          )
        : sessions

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
        <div className="flex flex-col gap-4">
            <div className="group/row flex items-center gap-6">
                <IconButton
                    size="sm"
                    variant="ghost"
                    icon={expanded ? 'chevron-down' : 'chevron-right'}
                    aria-label={expanded ? `Collapse ${folder}` : `Expand ${folder}`}
                    aria-expanded={expanded}
                    onClick={onToggle}
                />
                <button
                    type="button"
                    className="flex min-w-0 flex-1 items-center gap-6 text-left"
                    title={project.folder}
                    onClick={onToggle}
                >
                    <span className="truncate type-small text-fg">{folder}</span>
                    {!project.available && (
                        <Icon name="alert" size={12} tone="failed" aria-label="Folder unavailable" />
                    )}
                </button>
                <span className="flex shrink-0 items-center opacity-0 transition-opacity group-hover/row:opacity-100 group-focus-within/row:opacity-100">
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
                {expanded && sessions.length > 0 && (
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

            {expanded && (
                <div className="flex flex-col gap-2 pl-24">
                    {!project.available && (
                        <Notice tone="failed" compact icon="alert">
                            The folder is unavailable; history is readable, commands are refused.
                        </Notice>
                    )}
                    {sessions.length === 0 ? (
                        <p className="type-small text-faint">No sessions yet. New session starts one.</p>
                    ) : (
                        <>
                            {searchOpen && (
                                <label className="mb-4 flex h-30 items-center gap-8 rounded-lg border border-border bg-surface px-10 focus-within:border-border-hover">
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
                            {shown.length === 0 && (
                                <p className="px-6 type-small text-faint">No session matched.</p>
                            )}
                            {shown.map((session) => {
                                const title = session.title || 'New session'
                                const href = `/${agentId}/projects/${project.id}/chat/${session.topic}`
                                if (renamingTopic === session.topic) {
                                    return (
                                        <input
                                            key={session.topic}
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
                                            className="rounded-md border border-border bg-surface px-6 py-4 type-small text-fg outline-none focus:border-border-hover"
                                        />
                                    )
                                }
                                return (
                                    <div
                                        key={session.topic}
                                        className={`group/row relative flex items-center rounded-md hover:bg-raised ${activeTopic === session.topic ? 'bg-raised' : ''}`}
                                    >
                                        <button
                                            type="button"
                                            className="flex min-w-0 flex-1 items-center gap-6 px-6 py-4 text-left"
                                            onClick={() => navigate(href)}
                                        >
                                            {session.is_thinking && (
                                                <span
                                                    aria-label="Running"
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
                                            <span className="truncate type-small text-muted">{title}</span>
                                            {session.worktree && session.worktree.branch !== title && (
                                                <span className="shrink-0 truncate type-mono-xs text-faint">
                                                    {session.worktree.branch}
                                                </span>
                                            )}
                                        </button>
                                        <span className="absolute inset-y-0 right-2 flex items-center bg-raised opacity-0 transition-opacity group-hover/row:opacity-100 group-focus-within/row:opacity-100">
                                            <IconButton
                                                size="sm"
                                                variant="ghost"
                                                icon="pencil"
                                                aria-label={`Rename ${title}`}
                                                title="Rename session"
                                                onClick={() => {
                                                    setDraft(session.title || '')
                                                    setRenamingTopic(session.topic)
                                                }}
                                            />
                                            <IconButton
                                                size="sm"
                                                variant="ghost"
                                                icon="trash"
                                                className="enabled:hover:text-failed! enabled:active:text-failed!"
                                                aria-label={`Delete ${title}`}
                                                title="Delete conversation"
                                                onClick={() => setDeleting({ topic: session.topic, title })}
                                            />
                                        </span>
                                    </div>
                                )
                            })}
                        </>
                    )}
                </div>
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
