import { Dialog, Notice, Select, TextField } from '@vizier/design'
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

/** The server's rule for a worktree name, so a bad one is explained before anything is sent. */
const NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/

function nameProblem(name: string): string | null {
    if (!name) return null
    if (!NAME.test(name)) return 'Start with a letter or digit; then letters, digits, ".", "_" or "-" (64 at most).'
    if (name.endsWith('.lock') || name.includes('..')) return 'A name can\'t contain ".." or end in ".lock".'
    return null
}

/** Create one isolated worktree and open its chat (spec 041 US3). */
export function WorktreeDialog({ agentId, projectId, open, onClose }: Props) {
    const navigate = useNavigate()
    const refreshWorktrees = useProjects((s) => s.refreshWorktrees)
    const [name, setName] = useState('')
    const [branch, setBranch] = useState('')
    const [base, setBase] = useState('')
    const [branches, setBranches] = useState<GitBranch[]>([])
    const [loaded, setLoaded] = useState(false)
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        if (!open) return
        setName('')
        setBranch('')
        setLoaded(false)
        getGitBranches(agentId, projectId)
            .then((list) => {
                setBranches(list ?? [])
                setBase((current) => current || list?.find((b) => b.current)?.name || list?.[0]?.name || '')
            })
            .catch(() => setBranches([]))
            .finally(() => setLoaded(true))
    }, [open, agentId, projectId])

    const trimmed = name.trim()
    const problem = nameProblem(trimmed)
    const noGit = loaded && branches.length === 0
    const create = async () => {
        if (!trimmed || problem || !base || busy) return
        setBusy(true)
        try {
            const created = await createWorktree(agentId, projectId, {
                name: trimmed,
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

    const submitOnEnter = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.nativeEvent.isComposing) {
            e.preventDefault()
            void create()
        }
    }

    return (
        <Dialog
            open={open}
            title="New worktree"
            message="A separate checkout on its own branch, with one chat. The main folder is untouched."
            confirmLabel={busy ? 'Creating…' : 'Create worktree'}
            confirmIcon="plus"
            confirmDisabled={!trimmed || !!problem || !base}
            confirmLoading={busy}
            onConfirm={() => void create()}
            onCancel={onClose}
        >
            {noGit ? (
                <Notice>This folder isn't a git repository, or git isn't installed here.</Notice>
            ) : (
                <>
                    <TextField
                        label="Name"
                        value={name}
                        autoFocus
                        invalid={!!problem}
                        onChange={(e) => setName(e.target.value)}
                        onKeyDown={submitOnEnter}
                        helper={
                            problem ?? (
                                <span className="type-mono-xs">
                                    .vizier/{agentId}/{trimmed || 'name'}
                                </span>
                            )
                        }
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
                        onKeyDown={submitOnEnter}
                        placeholder={trimmed || 'Same as the name'}
                        helper="A new branch is created from the starting point."
                    />
                </>
            )}
        </Dialog>
    )
}
