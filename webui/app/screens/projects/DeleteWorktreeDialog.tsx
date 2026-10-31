import { Checkbox, Dialog } from '@vizier/design'
import { useEffect, useState } from 'react'
import {
    deleteWorktree,
    getWorktree,
    type WorktreeDetail,
} from '../../api/endpoints/projects'
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'

type Props = {
    agentId: string
    projectId: string
    worktree: { id: string; name: string; topic: string } | null
    onClose: () => void
    onDeleted: (topic: string) => void
}

/** Delete a worktree chat and its folder, saying what is lost (spec 041 US3). */
export function DeleteWorktreeDialog({ agentId, projectId, worktree, onClose, onDeleted }: Props) {
    const refreshWorktrees = useProjects((s) => s.refreshWorktrees)
    const [detail, setDetail] = useState<WorktreeDetail | null>(null)
    const [deleteBranch, setDeleteBranch] = useState(false)

    useEffect(() => {
        if (!worktree) {
            setDetail(null)
            return
        }
        getWorktree(agentId, projectId, worktree.id)
            .then((found) => {
                setDetail(found)
                setDeleteBranch(!!found.branch_merged)
            })
            .catch(() => setDetail(null))
    }, [agentId, projectId, worktree])

    if (!worktree) return null
    const dirty = !!detail && detail.changed + detail.untracked > 0
    const message = !detail
        ? undefined
        : !detail.available
          ? 'Its folder is already gone; deleting removes the chat and anything git still knows.'
          : dirty
            ? `${detail.changed} changed and ${detail.untracked} untracked file${detail.changed + detail.untracked === 1 ? '' : 's'} will be lost.`
            : 'The folder is clean.'

    const remove = async () => {
        try {
            await deleteWorktree(agentId, projectId, worktree.id, {
                force: !!dirty,
                deleteBranch,
            })
            await refreshWorktrees(agentId, projectId)
            onClose()
            onDeleted(worktree.topic)
        } catch (error) {
            toast.error(
                "Couldn't delete the worktree",
                error instanceof Error ? error.message : undefined
            )
        }
    }

    return (
        <Dialog
            open
            tone={dirty ? 'danger' : 'default'}
            title={`Delete ${detail?.name ?? worktree.name}?`}
            message={message}
            confirmLabel={dirty ? 'Delete and lose changes' : 'Delete'}
            confirmIcon="trash"
            onConfirm={() => void remove()}
            onCancel={onClose}
        >
            <Checkbox
                checked={deleteBranch}
                onChange={(e) => setDeleteBranch(e.target.checked)}
                label="Also delete the branch"
            />
        </Dialog>
    )
}
