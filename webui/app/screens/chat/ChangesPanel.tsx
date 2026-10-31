import { EmptyState, MiddleTruncate, PanelHeader } from '@vizier/design'
import { useEffect, useState, type ReactNode } from 'react'
import { getGitChanges, type GitChange } from '../../api/endpoints/projects'
import { projectKey, useProjects } from '../../stores/projects'
import { ChangesDialog, SIGN } from './ChangesDialog'

type Props = {
    agentId: string
    projectId: string
    topicId: string
    /** Changes when a turn ends, so the panel re-reads what the agent did. */
    refreshKey?: unknown
    /** The branch switcher, here on a phone where the header has no room for it. */
    branchAction?: ReactNode
}

/** Uncommitted changes in the chat's details panel; a file opens its diff in a dialog (spec 041 US4). */
export function ChangesPanel({ agentId, projectId, topicId, refreshKey, branchAction }: Props) {
    const summary = useProjects((s) => s.git[`${projectKey(agentId, projectId)}/${topicId}`])
    const [changes, setChanges] = useState<GitChange[]>([])
    const [selected, setSelected] = useState<string | null>(null)

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
                        return (
                            <li key={change.path}>
                                <button
                                    type="button"
                                    aria-haspopup="dialog"
                                    title={sign.label}
                                    onClick={() => setSelected(change.path)}
                                    className="flex w-full items-center gap-8 rounded-sm px-6 py-4 text-left hover:bg-raised"
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
            <ChangesDialog
                agentId={agentId}
                projectId={projectId}
                topicId={topicId}
                changes={changes}
                path={selected}
                onPath={setSelected}
            />
        </section>
    )
}
