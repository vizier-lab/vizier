import { Badge, Dialog, IconButton, Menu, MenuItem, NavItem, NavSection, TextField, useDismiss } from '@vizier/design'
import { useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import { ApiError } from '../../api/client'
import { createTopic, deleteTopic, deleteWorkspace, renameTopic, renameWorkspace, type WorkspaceSummary } from '../../api/endpoints/workspaces'
import { useWorkspaces } from '../../stores/workspaces'
import { toast } from '../../stores/toasts'

/** A topic name as the server will take it: lower-case letters, digits and dashes. */
const normalize = (typed: string) => typed.trim().toLowerCase()
const VALID = /^[a-z0-9][a-z0-9-]*$/

/** The column beside the rail in a Workspace: its name, its topics, and a new topic. */
export function WorkspaceSidebar({ workspace, activeTopic, onMembers }: { workspace: WorkspaceSummary; activeTopic?: string; onMembers: () => void }) {
    const navigate = useNavigate()
    const load = useWorkspaces((s) => s.load)
    const [adding, setAdding] = useState(false)
    const [name, setName] = useState('')
    const [saving, setSaving] = useState(false)
    // Owners manage the Workspace: rename or delete it, or the topic on screen.
    const [managing, setManaging] = useState<null | 'rename-topic' | 'delete-topic' | 'rename-workspace' | 'delete-workspace'>(null)
    const [menu, setMenu] = useState(false)
    const menuBox = useRef<HTMLDivElement>(null)
    useDismiss(menu, () => setMenu(false), menuBox)
    const isOwner = workspace.role === 'owner' || workspace.role === 'admin'
    const current = workspace.topics.find((t) => t.id === activeTopic)
    const key = normalize(name)
    const taken = workspace.topics.some((t) => t.name === key)
    const invalid = key.length > 0 && (!VALID.test(key) || key.length > 64)
    const problem = taken ? 'A topic with that name already exists.' : invalid ? 'Use lower-case letters, digits and dashes, up to 64.' : undefined

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
            if (managing === 'rename-topic' && current) await renameTopic(workspace.id, current.id, key)
            if (managing === 'rename-workspace') await renameWorkspace(workspace.id, key)
            if (managing === 'delete-topic' && current) {
                await deleteTopic(workspace.id, current.id)
                await load()
                const other = workspace.topics.find((t) => t.id !== current.id)
                if (other) navigate(`/workspaces/${workspace.id}/${other.id}`)
            }
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
    const renaming = managing === 'rename-topic' || managing === 'rename-workspace'
    const clash = managing === 'rename-topic' && workspace.topics.some((t) => t.name === key && t.id !== current?.id)
    const manageProblem = clash ? 'A topic with that name already exists.' : invalid ? 'Use lower-case letters, digits and dashes, up to 64.' : undefined

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
                                <Menu aria-label="Manage this Workspace">
                                    {current && (
                                        <MenuItem icon="pencil" label="Rename topic" onClick={() => { setMenu(false); setName(current.name); setManaging('rename-topic') }} />
                                    )}
                                    {current && workspace.topics.length > 1 && (
                                        <MenuItem icon="trash" tone="danger" label="Delete topic" onClick={() => { setMenu(false); setManaging('delete-topic') }} />
                                    )}
                                    <MenuItem icon="pencil" label="Rename Workspace" onClick={() => { setMenu(false); setName(workspace.name); setManaging('rename-workspace') }} />
                                    <MenuItem icon="trash" tone="danger" label="Delete Workspace" onClick={() => { setMenu(false); setManaging('delete-workspace') }} />
                                </Menu>
                            </div>
                        )}
                    </div>
                )}
            </div>
            <NavSection label="Topics">
                {workspace.topics.map((t) => (
                    <NavItem
                        key={t.id}
                        icon="chat"
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
                ))}
                <NavItem icon="plus" label="New topic" onSelect={() => setAdding(true)} />
            </NavSection>
            <Dialog
                open={managing !== null}
                title={
                    managing === 'rename-topic' ? 'Rename topic'
                    : managing === 'rename-workspace' ? 'Rename Workspace'
                    : managing === 'delete-topic' ? `Delete #${current?.name ?? ''}?`
                    : `Delete ${workspace.name}?`
                }
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
                        {managing === 'delete-topic'
                            ? 'Its messages and every agent\'s record of it are removed. This cannot be undone.'
                            : 'Every topic and message, and every agent\'s record of them, is removed. Running turns are stopped first. This cannot be undone.'}
                    </p>
                )}
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

