import { Dialog, Select, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import {
    createWorktree,
    getGitBranches,
    type GitBranch,
} from '../../api/endpoints/projects'
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'

type Props = {
    agentId: string
    projectId: string
    open: boolean
    onClose: () => void
}

/** Create one isolated worktree and open its chat (spec 041 US3). */
export function WorktreeDialog({ agentId, projectId, open, onClose }: Props) {
    const navigate = useNavigate()
    const refreshWorktrees = useProjects((s) => s.refreshWorktrees)
    const [name, setName] = useState('')
    const [branch, setBranch] = useState('')
    const [base, setBase] = useState('')
    const [branches, setBranches] = useState<GitBranch[]>([])
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        if (!open) return
        setName('')
        setBranch('')
        getGitBranches(agentId, projectId)
            .then((list) => {
                setBranches(list ?? [])
                setBase((current) => current || list?.find((b) => b.current)?.name || list?.[0]?.name || '')
            })
            .catch(() => setBranches([]))
    }, [open, agentId, projectId])

    const create = async () => {
        if (!name.trim() || !base) return
        setBusy(true)
        try {
            const created = await createWorktree(agentId, projectId, {
                name: name.trim(),
                base,
                branch: branch.trim() || undefined,
            })
            await refreshWorktrees(agentId, projectId)
            onClose()
            navigate(`/${agentId}/projects/${projectId}/chat/${created.topic}`)
        } catch (error) {
            toast.error(
                "Couldn't create the worktree",
                error instanceof Error ? error.message : undefined
            )
        } finally {
            setBusy(false)
        }
    }

    return (
        <Dialog
            open={open}
            title="New worktree"
            message="A separate checkout on its own branch, with one chat. The main folder is untouched."
            confirmLabel="Create worktree"
            confirmIcon="plus"
            onConfirm={() => void create()}
            onCancel={onClose}
        >
            <TextField
                label="Name"
                value={name}
                autoFocus
                onChange={(e) => setName(e.target.value)}
                helper="Letters, digits, '.', '_' and '-'. Becomes the folder name."
            />
            <Select<string>
                label="Start from"
                value={base}
                onChange={setBase}
                options={branches.map((b) => ({ value: b.name, label: b.name }))}
            />
            <TextField
                label="Branch"
                value={branch}
                onChange={(e) => setBranch(e.target.value)}
                placeholder={name || 'Same as the name'}
                helper={busy ? 'Creating…' : undefined}
            />
        </Dialog>
    )
}
