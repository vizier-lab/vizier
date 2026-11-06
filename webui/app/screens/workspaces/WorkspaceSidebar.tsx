import { Dialog, NavItem, NavSection, TextField } from '@vizier/design'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { ApiError } from '../../api/client'
import { createTopic, type WorkspaceSummary } from '../../api/endpoints/workspaces'
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

    return (
        <nav aria-label={`${workspace.name} topics`} className="flex h-full w-256 shrink-0 flex-col gap-4 bg-shell pt-12 pr-12 pl-0">
            <h2 className="flex h-52 items-center gap-8 px-14 type-title text-fg">
                <span className="min-w-0 truncate">{workspace.name}</span>
            </h2>
            <NavSection label="Topics">
                {workspace.topics.map((t) => (
                    <NavItem key={t.id} icon="chat" label={`#${t.name}`} href={`/workspaces/${workspace.id}/${t.id}`} active={t.id === activeTopic} />
                ))}
                <NavItem icon="plus" label="New topic" onSelect={() => setAdding(true)} />
            </NavSection>
            <NavSection label="People and agents">
                <NavItem icon="users" label="Members" onSelect={onMembers} />
            </NavSection>
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

