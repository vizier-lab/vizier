import { DiffView, MiddleTruncate, Notice } from '@vizier/design'
import { useEffect, useState } from 'react'
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
}

const SIGN: Record<GitChange['kind'], string> = {
    modified: 'M',
    added: 'A',
    deleted: 'D',
    renamed: 'R',
    untracked: '?',
    conflicted: '!',
}

/** Uncommitted changes and per-file diffs, in the chat's details panel (spec 041 US4). */
export function ChangesPanel({ agentId, projectId, topicId, refreshKey }: Props) {
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
        <section className="flex flex-col gap-10">
            <h3 className="type-ui-strong text-fg">Changes</h3>
            {changes.length === 0 ? (
                <p className="type-small text-faint">No uncommitted changes.</p>
            ) : (
                <ul className="flex flex-col gap-2">
                    {changes.map((change) => (
                        <li key={change.path}>
                            <button
                                type="button"
                                onClick={() => setSelected(change.path)}
                                className="flex w-full items-center gap-8 rounded-sm px-6 py-4 text-left hover:bg-raised"
                            >
                                <span className="type-mono-xs text-faint">{SIGN[change.kind]}</span>
                                <MiddleTruncate text={change.path} className="min-w-0 flex-1 type-small text-soft" />
                                <span className="type-mono-xs text-live-text">+{change.added}</span>
                                <span className="type-mono-xs text-failed-text">−{change.removed}</span>
                            </button>
                        </li>
                    ))}
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
