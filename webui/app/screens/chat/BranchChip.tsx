import { Button, Floating, Menu, MenuDivider, MenuItem, useDismiss } from '@vizier/design'
import { useEffect, useRef, useState } from 'react'
import {
    getGitBranches,
    switchBranch,
    type GitBranch,
} from '../../api/endpoints/projects'
import { projectKey, useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'

type Props = { agentId: string; projectId: string; topicId: string }

/** Past this many branches the menu gets a filter field (the design Menu's own rule). */
const SEARCH_FROM = 8

/** The chat header's branch and change count, with the branch switcher (spec 041 US4). */
export function BranchChip({ agentId, projectId, topicId }: Props) {
    const summary = useProjects((s) => s.git[`${projectKey(agentId, projectId)}/${topicId}`])
    const refreshGit = useProjects((s) => s.refreshGit)
    const [open, setOpen] = useState(false)
    const [branches, setBranches] = useState<GitBranch[] | null>(null)
    const [query, setQuery] = useState('')
    const [creating, setCreating] = useState(false)
    const [name, setName] = useState('')
    const anchor = useRef<HTMLDivElement>(null)
    const pop = useRef<HTMLDivElement>(null)
    const close = () => {
        setOpen(false)
        setCreating(false)
        setQuery('')
        setName('')
    }
    useDismiss(open, close, pop, anchor)

    useEffect(() => {
        if (!open) return
        setBranches(null)
        getGitBranches(agentId, projectId, topicId)
            .then((list) => setBranches(list ?? []))
            .catch(() => setBranches([]))
    }, [open, agentId, projectId, topicId])

    if (!summary?.available || !summary.branch) return null

    const switchTo = (branch: string, create = false) => {
        close()
        switchBranch(agentId, projectId, { branch, create, topic: topicId })
            .then(() => refreshGit(agentId, projectId, topicId))
            .catch((error) =>
                toast.error(
                    "Couldn't switch branch",
                    error instanceof Error ? error.message : undefined
                )
            )
    }

    const shown = (branches ?? []).filter((b) =>
        b.name.toLowerCase().includes(query.trim().toLowerCase())
    )

    return (
        <div ref={anchor}>
            <Button
                size="sm"
                icon="git-branch"
                aria-expanded={open}
                aria-haspopup="menu"
                title={summary.detached ? 'Detached HEAD' : 'Switch branch'}
                onClick={() => (open ? close() : setOpen(true))}
            >
                <span className="max-w-160 truncate">{summary.branch}</span>
                {summary.changed > 0 && (
                    <span className="type-mono-xs text-faint">· {summary.changed}</span>
                )}
            </Button>
            {open && (
                <Floating anchor={anchor} align="end">
                    <div ref={pop}>
                        <Menu
                            aria-label="Branches"
                            search={
                                (branches?.length ?? 0) > SEARCH_FROM
                                    ? { value: query, onChange: setQuery, placeholder: 'Find a branch' }
                                    : undefined
                            }
                        >
                            {branches === null ? (
                                <p className="px-8 py-6 type-small text-faint">Loading branches…</p>
                            ) : shown.length === 0 ? (
                                <p className="px-8 py-6 type-small text-faint">No branch matches.</p>
                            ) : (
                                shown.map((branch) => (
                                    <MenuItem
                                        key={branch.name}
                                        label={branch.name}
                                        selected={branch.current}
                                        meta={
                                            branch.checked_out_in
                                                ? `in ${branch.checked_out_in.name}`
                                                : undefined
                                        }
                                        title={
                                            branch.checked_out_in
                                                ? `Checked out in worktree ${branch.checked_out_in.name}`
                                                : undefined
                                        }
                                        disabled={!!branch.checked_out_in}
                                        onClick={() => !branch.current && switchTo(branch.name)}
                                    />
                                ))
                            )}
                            <MenuDivider />
                            {creating ? (
                                <form
                                    className="flex flex-col gap-6 p-4"
                                    onSubmit={(e) => {
                                        e.preventDefault()
                                        if (name.trim()) switchTo(name.trim(), true)
                                    }}
                                >
                                    <label className="flex h-29 items-center gap-8 rounded-md bg-ground px-10 focus-within:outline focus-within:outline-live">
                                        <input
                                            autoFocus
                                            value={name}
                                            onChange={(e) => setName(e.target.value)}
                                            placeholder="Branch name"
                                            aria-label={`New branch from ${summary.branch}`}
                                            className="min-w-0 flex-1 bg-transparent type-code text-fg outline-none placeholder:text-faint"
                                        />
                                    </label>
                                    <p className="px-2 type-mono-xs text-faint">
                                        From {summary.branch} · Enter to create
                                    </p>
                                </form>
                            ) : (
                                <MenuItem
                                    icon="plus"
                                    label={`New branch from ${summary.branch}`}
                                    onClick={() => setCreating(true)}
                                />
                            )}
                        </Menu>
                    </div>
                </Floating>
            )}
        </div>
    )
}
