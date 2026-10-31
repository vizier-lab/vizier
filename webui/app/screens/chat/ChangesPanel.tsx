import { DiffView, EmptyState, MiddleTruncate, Notice, PanelHeader } from '@vizier/design'
import { useEffect, useState, type ReactNode } from 'react'
import {
    getGitChanges,
    getGitDiff,
    type GitChange,
    type GitFileDiff,
} from '../../api/endpoints/projects'
import { parseUnifiedDiff } from '../../lib/diff'
import { projectKey, useProjects } from '../../stores/projects'

type Props = {
    agentId: string
    projectId: string
    topicId: string
    /** Changes when a turn ends, so the panel re-reads what the agent did. */
    refreshKey?: unknown
    /** The branch switcher, here on a phone where the header has no room for it. */
    branchAction?: ReactNode
}

const SIGN: Record<GitChange['kind'], { letter: string; label: string; tone: string }> = {
    modified: { letter: 'M', label: 'Modified', tone: 'text-muted' },
    added: { letter: 'A', label: 'Added', tone: 'text-live-text' },
    deleted: { letter: 'D', label: 'Deleted', tone: 'text-failed-text' },
    renamed: { letter: 'R', label: 'Renamed', tone: 'text-muted' },
    untracked: { letter: '?', label: 'Untracked', tone: 'text-faint' },
    conflicted: { letter: '!', label: 'Conflicted', tone: 'text-failed-text' },
}

/** Uncommitted changes and per-file diffs, in the chat's details panel (spec 041 US4). */
export function ChangesPanel({ agentId, projectId, topicId, refreshKey, branchAction }: Props) {
    const summary = useProjects((s) => s.git[`${projectKey(agentId, projectId)}/${topicId}`])
    const [changes, setChanges] = useState<GitChange[]>([])
    const [selected, setSelected] = useState<string | null>(null)
    const [diff, setDiff] = useState<GitFileDiff | null>(null)

    useEffect(() => {
        if (!summary?.available) return
        let alive = true
        getGitChanges(agentId, projectId, topicId)
            .then((list) => {
                if (alive) setChanges(list ?? [])
            })
            .catch(() => {
                if (alive) setChanges([])
            })
        return () => {
            alive = false
        }
    }, [agentId, projectId, topicId, summary?.available, refreshKey])

    useEffect(() => {
        if (!selected) {
            setDiff(null)
            return
        }
        getGitDiff(agentId, projectId, selected, topicId)
            .then(setDiff)
            .catch(() => setDiff(null))
    }, [agentId, projectId, topicId, selected])

    if (!summary?.available) return null

    return (
        <section aria-label="Changes" className="flex flex-col gap-8">
            <PanelHeader
                title="Changes"
                meta={changes.length === 1 ? '1 file' : `${changes.length} files`}
                action={branchAction}
                level={3}
            />
            {changes.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="check"
                    title="Nothing uncommitted"
                    message="Files the agent changes appear here, with their diffs."
                />
            ) : (
                <ul className="flex flex-col gap-2">
                    {changes.map((change) => {
                        const sign = SIGN[change.kind]
                        const open = selected === change.path
                        return (
                            <li key={change.path}>
                                <button
                                    type="button"
                                    aria-expanded={open}
                                    title={sign.label}
                                    onClick={() => setSelected(open ? null : change.path)}
                                    className={`flex w-full items-center gap-8 rounded-sm px-6 py-4 text-left hover:bg-raised ${open ? 'bg-raised' : ''}`}
                                >
                                    <span
                                        aria-label={sign.label}
                                        className={`w-8 shrink-0 text-center type-mono-xs ${sign.tone}`}
                                    >
                                        {sign.letter}
                                    </span>
                                    <MiddleTruncate
                                        text={change.path}
                                        className="min-w-0 flex-1 type-small text-soft"
                                    />
                                    <span className="type-mono-xs text-live-text">+{change.added}</span>
                                    <span className="type-mono-xs text-failed-text">−{change.removed}</span>
                                </button>
                            </li>
                        )
                    })}
                </ul>
            )}
            {diff && (
                <div className="max-h-420 overflow-auto">
                    {diff.binary ? (
                        <Notice compact>Binary file; no text diff.</Notice>
                    ) : (
                        <DiffView title={diff.path} lines={parseUnifiedDiff(diff.diff)} />
                    )}
                    {diff.truncated && <Notice compact>The diff was cut off.</Notice>}
                </div>
            )}
        </section>
    )
}
