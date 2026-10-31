import { Button, Icon, IconButton, Notice, TextField } from '@vizier/design'
import { useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import {
    createProjectSession,
    updateProject,
    type Project,
    type ProjectSessionRow,
} from '../../api/endpoints/projects'
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'
import { RemoveProjectDialog } from './RemoveProjectDialog'

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
    canManage: boolean
    onToggle: () => void
}

/**
 * One project above Sessions (FR-005): its name, folder, availability and session count, and
 * — expanded — the caller's sessions plus new-session, rename, shell switch and remove.
 */
export function ProjectRow({ agentId, project, sessions, expanded, canManage, onToggle }: Props) {
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const activeTopic = pathname.startsWith(`/${agentId}/projects/${project.id}/chat/`)
        ? decodeURIComponent(pathname.split('/').pop() ?? '')
        : ''
    const insertSession = useProjects((s) => s.insertSession)
    const loadSessions = useProjects((s) => s.loadSessions)
    const refresh = useProjects((s) => s.refresh)
    const [renaming, setRenaming] = useState(false)
    const [name, setName] = useState(project.name)
    const [busy, setBusy] = useState(false)
    const [removing, setRemoving] = useState(false)

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

    const rename = async () => {
        const trimmed = name.trim()
        if (trimmed.length < 1 || trimmed.length > 120) {
            toast.error('The name must be 1–120 characters')
            return
        }
        setBusy(true)
        try {
            await updateProject(agentId, project.id, { name: trimmed })
            setRenaming(false)
            await refresh(agentId)
        } catch (e) {
            toast.error("Couldn't rename the project", e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    const toggleShell = async () => {
        setBusy(true)
        try {
            await updateProject(agentId, project.id, { shell_enabled: !project.shell_enabled })
            await refresh(agentId)
        } catch (e) {
            toast.error("Couldn't change the shell switch", e instanceof Error ? e.message : undefined)
        } finally {
            setBusy(false)
        }
    }

    return (
        <div className="flex flex-col gap-4">
            <div className="flex items-center gap-6">
                <IconButton
                    size="sm"
                    variant="ghost"
                    icon={expanded ? 'chevron-down' : 'chevron-right'}
                    aria-label={expanded ? `Collapse ${project.name}` : `Expand ${project.name}`}
                    aria-expanded={expanded}
                    onClick={onToggle}
                />
                <button
                    type="button"
                    className="flex min-w-0 flex-1 flex-col items-start gap-1 text-left"
                    onClick={onToggle}
                >
                    <span className="flex w-full items-center gap-6">
                        <span className="truncate type-small text-fg">{project.name}</span>
                        {!project.available && (
                            <Icon name="alert" size={12} tone="failed" aria-label="Folder unavailable" />
                        )}
                        {!project.shell_enabled && (
                            <Icon name="lock" size={12} tone="faint" aria-label="Workspace shell off" />
                        )}
                    </span>
                    <span className="truncate type-mono-xs text-faint">{folderName(project.folder)}</span>
                </button>
                <span className="type-mono-xs text-faint">{project.session_count}</span>
                <IconButton
                    size="sm"
                    variant="ghost"
                    icon="plus"
                    aria-label={`New session in ${project.name}`}
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
                        sessions.map((session) => (
                            <button
                                key={session.topic}
                                type="button"
                                className={`flex min-w-0 items-center gap-6 rounded-md px-6 py-4 text-left hover:bg-raised ${activeTopic === session.topic ? 'bg-raised' : ''}`}
                                onClick={() =>
                                    navigate(`/${agentId}/projects/${project.id}/chat/${session.topic}`)
                                }
                            >
                                {session.is_thinking && (
                                    <span
                                        aria-label="Running"
                                        className="size-6 shrink-0 rounded-pill bg-live"
                                    />
                                )}
                                <span className="truncate type-small text-muted">
                                    {session.title || 'New session'}
                                </span>
                            </button>
                        ))
                    )}

                    {renaming ? (
                        <div className="flex items-end gap-6 pt-4">
                            <TextField
                                label="Name"
                                value={name}
                                onChange={(e) => setName(e.target.value)}
                                className="min-w-0 flex-1"
                            />
                            <Button size="sm" loading={busy} onClick={() => void rename()}>
                                Save
                            </Button>
                            <Button size="sm" onClick={() => setRenaming(false)} disabled={busy}>
                                Cancel
                            </Button>
                        </div>
                    ) : (
                        canManage && (
                            <div className="flex flex-wrap items-center gap-4 pt-4">
                                <Button
                                    size="sm"
                                    onClick={() => {
                                        setName(project.name)
                                        setRenaming(true)
                                    }}
                                >
                                    Rename
                                </Button>
                                <Button size="sm" disabled={busy} onClick={() => void toggleShell()}>
                                    {project.shell_enabled ? 'Turn shell off' : 'Turn shell on'}
                                </Button>
                                <Button size="sm" variant="destructive" onClick={() => setRemoving(true)}>
                                    Remove
                                </Button>
                                <button
                                    type="button"
                                    className="type-mono-xs text-faint underline"
                                    onClick={() => void loadSessions(agentId, project.id)}
                                >
                                    Refresh sessions
                                </button>
                            </div>
                        )
                    )}
                </div>
            )}

            <RemoveProjectDialog
                agentId={agentId}
                project={removing ? { id: project.id, name: project.name } : null}
                onClose={() => setRemoving(false)}
            />
        </div>
    )
}
