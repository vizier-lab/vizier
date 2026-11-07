import { Badge, Dialog, IconButton, Icon, Menu, MenuItem, NavItem, NavSection, TextField, useDismiss } from '@vizier/design'
import { useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import { ApiError } from '../../api/client'
import { createTopic, deleteTopic, deleteWorkspace, renameTopic, renameWorkspace, type WorkspaceSummary } from '../../api/endpoints/workspaces'
import { normalizeTopicName, topicNameProblem } from '../../lib/workspaces'
import { useWorkspaces } from '../../stores/workspaces'
import { toast } from '../../stores/toasts'

// The row's small buttons, as in an agent's session list: on hover and focus, and always where
// there is no hover to reveal them.
const ROW_BUTTON =
    'absolute top-1/2 grid size-24 -translate-y-1/2 place-items-center rounded-md text-faint opacity-0 transition-[opacity,color,background-color] hover:bg-raised focus-visible:opacity-100 group-hover/row:opacity-100 group-focus-within/row:opacity-100 [@media(hover:none)]:opacity-100'

/** The column beside the rail in a Workspace: its name, its topics, and a new topic. */
export function WorkspaceSidebar({ workspace, activeTopic, onMembers }: { workspace: WorkspaceSummary; activeTopic?: string; onMembers: () => void }) {
    const navigate = useNavigate()
    const load = useWorkspaces((s) => s.load)
    const [adding, setAdding] = useState(false)
    const [name, setName] = useState('')
    const [saving, setSaving] = useState(false)
    // Owners manage the Workspace: rename or delete it, or the topic on screen.
    const [managing, setManaging] = useState<null | 'rename-workspace' | 'delete-workspace'>(null)
    const [menu, setMenu] = useState(false)
    const menuBox = useRef<HTMLDivElement>(null)
    useDismiss(menu, () => setMenu(false), menuBox)
    const isOwner = workspace.role === 'owner' || workspace.role === 'admin'
    const [renamingTopic, setRenamingTopic] = useState<string | null>(null)
    const [draft, setDraft] = useState('')
    const [deletingTopic, setDeletingTopic] = useState<{ id: string; name: string } | null>(null)
    const key = normalizeTopicName(name)
    const problem = topicNameProblem(name, workspace.topics)
    const draftProblem = topicNameProblem(draft, workspace.topics, renamingTopic ?? undefined)

    const add = async () => {
        setSaving(true)
        try {
            const topic = await createTopic(workspace.id, key)
            await load()
            setAdding(false)
            setName('')
            navigate(`/workspaces/${workspace.id}/${topic.id}`)
        } catch (e) {
            toast.error(e instanceof ApiError ? e.message : "Couldn't add the topic")
        } finally {
            setSaving(false)
        }
    }

    const manage = async () => {
        if (!managing) return
        setSaving(true)
        try {
            if (managing === 'rename-workspace') await renameWorkspace(workspace.id, key)
            if (managing === 'delete-workspace') {
                await deleteWorkspace(workspace.id)
                await load()
                navigate('/', { replace: true })
            }
            await load()
            setManaging(null)
            setName('')
        } catch (e) {
            toast.error(e instanceof ApiError ? e.message : "Couldn't do that")
        } finally {
            setSaving(false)
        }
    }
    const renaming = managing === 'rename-workspace'
    const manageProblem = key && (!/^[a-z0-9][a-z0-9-]*$/.test(key) || key.length > 64) ? 'Use lower-case letters, digits and dashes, up to 64.' : undefined

    // Enter commits and Escape or leaving the row cancels, as when renaming a session.
    const commitRename = async (id: string, current: string) => {
        const next = normalizeTopicName(draft)
        if (!next || next === current) return setRenamingTopic(null)
        if (draftProblem) return toast.error(draftProblem)
        setRenamingTopic(null)
        try {
            await renameTopic(workspace.id, id, next)
            await load()
        } catch (e) {
            toast.error(e instanceof ApiError ? e.message : "Couldn't rename the topic")
        }
    }

    const removeTopic = async () => {
        if (!deletingTopic) return
        setSaving(true)
        try {
            await deleteTopic(workspace.id, deletingTopic.id)
            await load()
            // The topic on screen is gone: go to the first one left.
            if (deletingTopic.id === activeTopic) {
                const other = workspace.topics.find((t) => t.id !== deletingTopic.id)
                if (other) navigate(`/workspaces/${workspace.id}/${other.id}`, { replace: true })
            }
            setDeletingTopic(null)
        } catch (e) {
            toast.error(e instanceof ApiError ? e.message : "Couldn't delete the topic")
        } finally {
            setSaving(false)
        }
    }

    return (
        <nav aria-label={`${workspace.name} topics`} className="flex h-full w-256 shrink-0 flex-col gap-4 bg-shell pt-12 pr-12 pl-0">
            <div className="flex h-52 items-center gap-4 px-14">
                <h2 className="min-w-0 flex-1 truncate type-title text-fg">{workspace.name}</h2>
                <IconButton icon="users" size="sm" aria-label={`Members of ${workspace.name}`} onClick={onMembers} />
                {isOwner && (
                    <div ref={menuBox} className="relative">
                        <IconButton icon="more" size="sm" aria-label="Manage this Workspace" aria-haspopup="menu" aria-expanded={menu} onClick={() => setMenu((m) => !m)} />
                        {menu && (
                            <div className="absolute top-full right-0 z-30 mt-4">
                                <Menu aria-label="Manage this Workspace" className="w-192!">
                                    <MenuItem icon="pencil" label="Rename Workspace" onClick={() => { setMenu(false); setName(workspace.name); setManaging('rename-workspace') }} />
                                    <MenuItem icon="trash" tone="danger" label="Delete Workspace" onClick={() => { setMenu(false); setManaging('delete-workspace') }} />
                                </Menu>
                            </div>
                        )}
                    </div>
                )}
            </div>
            <NavSection label="Topics">
                {workspace.topics.map((t) => {
                    const deletable = isOwner && workspace.topics.length > 1
                    if (renamingTopic === t.id) {
                        return (
                            <input
                                key={t.id}
                                autoFocus
                                aria-label="Topic name"
                                aria-invalid={!!draftProblem}
                                title={draftProblem}
                                value={draft}
                                maxLength={64}
                                onChange={(e) => setDraft(e.target.value)}
                                onBlur={() => setRenamingTopic(null)}
                                onKeyDown={(e) => {
                                    if (e.key === 'Enter') void commitRename(t.id, t.name)
                                    if (e.key === 'Escape') setRenamingTopic(null)
                                }}
                                className={`h-31 w-full rounded-md border bg-surface px-10 type-body text-fg outline-none ${draftProblem ? 'border-failed' : 'border-border-hover'}`}
                            />
                        )
                    }
                    return (
                        <div key={t.id} className="group/row relative">
                            {/* The focus outline stays inside the row: the column clips at its left edge. */}
                            <NavItem
                                className={`focus-visible:-outline-offset-2 ${isOwner ? (deletable ? 'group-hover/row:pr-60 group-focus-within/row:pr-60 [@media(hover:none)]:pr-60' : 'group-hover/row:pr-36 group-focus-within/row:pr-36 [@media(hover:none)]:pr-36') : ''}`}
                                label={
                                    <span className="flex items-center gap-8">
                                        <span className="min-w-0 flex-1 truncate"># {t.name}</span>
                                        {t.unread > 0 && <Badge tone="rank" aria-label={`${t.unread} unread`}>{t.unread}</Badge>}
                                    </span>
                                }
                                href={`/workspaces/${workspace.id}/${t.id}`}
                                active={t.id === activeTopic}
                                live={workspace.working && t.id === activeTopic}
                            />
                            {isOwner && (
                                <button
                                    type="button"
                                    aria-label={`Rename ${t.name}`}
                                    title="Rename topic"
                                    onClick={() => {
                                        setDraft(t.name)
                                        setRenamingTopic(t.id)
                                    }}
                                    className={`${ROW_BUTTON} hover:text-fg ${deletable ? 'right-30' : 'right-6'}`}
                                >
                                    <Icon name="pencil" size={13} />
                                </button>
                            )}
                            {deletable && (
                                <button
                                    type="button"
                                    aria-label={`Delete ${t.name}`}
                                    title="Delete topic"
                                    onClick={() => setDeletingTopic({ id: t.id, name: t.name })}
                                    className={`${ROW_BUTTON} right-6 hover:text-failed`}
                                >
                                    <Icon name="trash" size={13} />
                                </button>
                            )}
                        </div>
                    )
                })}
                <NavItem icon="plus" label="New topic" onSelect={() => setAdding(true)} />
            </NavSection>
            <Dialog
                open={managing !== null}
                title={renaming ? 'Rename Workspace' : `Delete ${workspace.name}?`}
                confirmLabel={renaming ? 'Rename' : 'Delete'}
                confirmDisabled={renaming && (!key || !!manageProblem)}
                confirmLoading={saving}
                onConfirm={() => void manage()}
                onCancel={() => setManaging(null)}
            >
                {renaming ? (
                    <TextField
                        label="Name"
                        mono
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        invalid={!!manageProblem}
                        helper={manageProblem ?? 'Agents type this name, so keep it short.'}
                        autoFocus
                        onKeyDown={(e) => e.key === 'Enter' && key && !manageProblem && void manage()}
                    />
                ) : (
                    <p className="type-ui text-fg">
                        Every topic and message, and every agent's record of them, is removed. Running turns are stopped first. This cannot be undone.
                    </p>
                )}
            </Dialog>
            <Dialog
                open={deletingTopic !== null}
                title={`Delete # ${deletingTopic?.name ?? ''}?`}
                confirmLabel="Delete"
                confirmLoading={saving}
                onConfirm={() => void removeTopic()}
                onCancel={() => setDeletingTopic(null)}
            >
                <p className="type-ui text-fg">Its messages and every agent's record of it are removed. This cannot be undone.</p>
            </Dialog>
            <Dialog
                open={adding}
                title="New topic"
                confirmLabel="Add topic"
                confirmDisabled={!key || !!problem}
                confirmLoading={saving}
                onConfirm={() => void add()}
                onCancel={() => setAdding(false)}
            >
                <TextField
                    label="Name"
                    mono
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    invalid={!!problem}
                    helper={problem ?? 'Agents type this name, so keep it short.'}
                    autoFocus
                    onKeyDown={(e) => e.key === 'Enter' && key && !problem && void add()}
                />
            </Dialog>
        </nav>
    )
}

