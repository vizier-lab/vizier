import { Dialog, TextField } from '@vizier/design'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { ApiError } from '../../api/client'
import { createWorkspace } from '../../api/endpoints/workspaces'
import { useWorkspaces } from '../../stores/workspaces'
import { toast } from '../../stores/toasts'

const VALID = /^[a-z0-9][a-z0-9-]*$/

/** What the server will make of a typed name: trimmed and lower-cased, as the dialog shows it. */
export const workspaceKey = (typed: string) => typed.trim().toLowerCase()

/** Names a new Workspace. A clash with an existing one shows as you type. */
export function NewWorkspaceDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
    const navigate = useNavigate()
    const { list, load } = useWorkspaces()
    const [name, setName] = useState('')
    const [saving, setSaving] = useState(false)
    const key = workspaceKey(name)
    const clash = list.some((w) => w.name === key)
    const invalid = key.length > 0 && (!VALID.test(key) || key.length > 64)
    const problem = clash
        ? 'You are already in a Workspace with that name. (Names are unique across the installation.)'
        : invalid
          ? 'Use lower-case letters, digits and dashes, up to 64.'
          : undefined

    const create = async () => {
        setSaving(true)
        try {
            const made = await createWorkspace(key)
            await load()
            setName('')
            onClose()
            navigate(`/workspaces/${made.id}/${made.topics[0]?.id ?? ''}`)
        } catch (e) {
            // Another installation-wide name the person cannot see in their own list.
            toast.error(e instanceof ApiError ? e.message : "Couldn't create the Workspace")
        } finally {
            setSaving(false)
        }
    }

    return (
        <Dialog
            open={open}
            title="New workspace"
            message="A shared space for people and agents. Agents read along and answer only when mentioned."
            confirmLabel="Create workspace"
            confirmDisabled={!key || !!problem}
            confirmLoading={saving}
            onConfirm={() => void create()}
            onCancel={onClose}
        >
            <TextField
                label="Name"
                mono
                value={name}
                onChange={(e) => setName(e.target.value)}
                invalid={!!problem}
                helper={problem ?? 'Agents address a Workspace by this name.'}
                autoFocus
                onKeyDown={(e) => e.key === 'Enter' && key && !problem && void create()}
            />
        </Dialog>
    )
}
