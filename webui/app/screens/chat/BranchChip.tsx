import { Button, Menu, MenuItem, useDismiss } from '@vizier/design'
import { useEffect, useRef, useState } from 'react'
import {
    getGitBranches,
    switchBranch,
    type GitBranch,
} from '../../api/endpoints/projects'
import { projectKey, useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'

type Props = { agentId: string; projectId: string; topicId: string }

/** The chat header's branch and change count, with the branch switcher (spec 041 US4). */
export function BranchChip({ agentId, projectId, topicId }: Props) {
    const summary = useProjects((s) => s.git[`${projectKey(agentId, projectId)}/${topicId}`])
    const refreshGit = useProjects((s) => s.refreshGit)
    const [open, setOpen] = useState(false)
    const [branches, setBranches] = useState<GitBranch[]>([])
    const ref = useRef<HTMLDivElement>(null)
    useDismiss(open, () => setOpen(false), ref)

    useEffect(() => {
        if (!open) return
        getGitBranches(agentId, projectId, topicId)
            .then((list) => setBranches(list ?? []))
            .catch(() => setBranches([]))
    }, [open, agentId, projectId, topicId])

    if (!summary?.available || !summary.branch) return null

    const switchTo = (branch: string, create = false) => {
        setOpen(false)
        switchBranch(agentId, projectId, { branch, create, topic: topicId })
            .then(() => refreshGit(agentId, projectId, topicId))
            .catch((error) =>
                toast.error(
                    "Couldn't switch branch",
                    error instanceof Error ? error.message : undefined
                )
            )
    }

    return (
        <div ref={ref} className="relative">
            <Button
                size="sm"
                icon="history"
                aria-expanded={open}
                onClick={() => setOpen((v) => !v)}
            >
                {summary.branch}
                {summary.changed > 0 ? ` · ${summary.changed} changed` : ''}
            </Button>
            {open && (
                <div className="absolute right-0 top-40 z-30">
                    <Menu aria-label="Branches">
                        {branches.map((branch) => (
                            <MenuItem
                                key={branch.name}
                                label={branch.name}
                                selected={branch.current}
                                icon={branch.current ? 'check' : undefined}
                                meta={branch.checked_out_in ? `in worktree ${branch.checked_out_in.name}` : undefined}
                                disabled={!!branch.checked_out_in}
                                onClick={() => switchTo(branch.name)}
                            />
                        ))}
                        <MenuItem
                            icon="plus"
                            label="New branch from …"
                            onClick={() => {
                                const name = window.prompt('New branch name')
                                if (name?.trim()) switchTo(name.trim(), true)
                            }}
                        />
                    </Menu>
                </div>
            )}
        </div>
    )
}
