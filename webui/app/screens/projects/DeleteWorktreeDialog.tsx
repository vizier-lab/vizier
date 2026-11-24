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
    const [busy, setBusy] = useState(false)
    // Changes that appeared while the dialog was open: the form turns destructive.
    const [forced, setForced] = useState(false)

    useEffect(() => {
        if (!worktree) {
            setDetail(null)
            setForced(false)
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
    const dirty = forced || (!!detail && detail.changed + detail.untracked > 0)
    const lost = detail ? detail.changed + detail.untracked : 0
    const message = !detail
        ? undefined
        : !detail.available
          ? 'Its folder is already gone; deleting removes the chat and anything git still knows.'
          : dirty
            ? lost > 0
                ? `${lost} file${lost === 1 ? '' : 's'} with uncommitted work will be lost (${detail.changed} changed, ${detail.untracked} untracked).`
                : 'The folder now has changes that will be lost.'
            : 'The folder is clean, so nothing is lost.'

    const remove = async () => {
        if (busy) return
        setBusy(true)
        try {
            await deleteWorktree(agentId, projectId, worktree.id, {
                force: !!dirty,
                deleteBranch,
            })
            await refreshWorktrees(agentId, projectId)
            onClose()
            onDeleted(worktree.topic)
        } catch (error) {
            const text = error instanceof Error ? error.message : ''
            // Something changed in the folder since the counts were read: say so, and ask again.
            if (/worktree_dirty|uncommitted|changes/i.test(text) && !dirty) {
                setForced(true)
                getWorktree(agentId, projectId, worktree.id).then(setDetail).catch(() => {})
            } else {
                toast.error("Couldn't delete the worktree", text || undefined)
            }
        } finally {
            setBusy(false)
        }
    }

    return (
        <Dialog
            open
            tone={dirty ? 'danger' : 'default'}
            title={`Delete ${detail?.name ?? worktree.name}?`}
            message={message}
            confirmLabel={busy ? 'Deleting…' : dirty ? 'Delete and lose changes' : 'Delete'}
            confirmIcon="trash"
            confirmDisabled={!detail}
            confirmLoading={busy}
            onConfirm={() => void remove()}
            onCancel={onClose}
        >
            <Checkbox
                checked={deleteBranch}
                onChange={(e) => setDeleteBranch(e.target.checked)}
                label={
                    detail
                        ? `Also delete the branch ${detail.branch}`
                        : 'Also delete the branch'
                }
            />
            {detail && deleteBranch && !detail.branch_merged && (
                <p className="type-small text-failed-text">
                    This branch isn't merged into {detail.base_ref}; its commits will be lost.
                </p>
            )}
        </Dialog>
    )
}
