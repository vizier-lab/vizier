import { DiffView, EmptyState, IconButton, MiddleTruncate, Modal, Notice, Skeleton } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { getGitDiff, type GitChange, type GitFileDiff } from '../../api/endpoints/projects'
import { parseUnifiedDiff } from '../../lib/diff'
import { useMobile } from '../../lib/useMobile'

export const SIGN: Record<GitChange['kind'], { letter: string; label: string; tone: string }> = {
    modified: { letter: 'M', label: 'Modified', tone: 'text-muted' },
    added: { letter: 'A', label: 'Added', tone: 'text-live-text' },
    deleted: { letter: 'D', label: 'Deleted', tone: 'text-failed-text' },
    renamed: { letter: 'R', label: 'Renamed', tone: 'text-muted' },
    untracked: { letter: '?', label: 'Untracked', tone: 'text-faint' },
    conflicted: { letter: '!', label: 'Conflicted', tone: 'text-failed-text' },
}

type Props = {
    agentId: string
    projectId: string
    topicId: string
    changes: GitChange[]
    /** The file shown; null closes the dialog. */
    path: string | null
    onPath: (path: string | null) => void
}

/** A floating review of the uncommitted changes: the file list beside the selected file's diff. */
export function ChangesDialog({ agentId, projectId, topicId, changes, path, onPath }: Props) {
    const mobile = useMobile()
    const [diff, setDiff] = useState<GitFileDiff | null>(null)
    const [failed, setFailed] = useState(false)

    const index = changes.findIndex((c) => c.path === path)
    const change = index >= 0 ? changes[index] : null
    const open = change !== null
    const step = (by: number) => {
        if (changes.length === 0) return
        onPath(changes[(index + by + changes.length) % changes.length].path)
    }

    useEffect(() => {
        if (!path) {
            setDiff(null)
            return
        }
        let alive = true
        setDiff(null)
        setFailed(false)
        getGitDiff(agentId, projectId, path, topicId)
            .then((d) => alive && setDiff(d))
            .catch(() => alive && setFailed(true))
        return () => {
            alive = false
        }
    }, [agentId, projectId, topicId, path])

    useEffect(() => {
        if (!open) return
        const onKey = (e: KeyboardEvent) => {
            const el = e.target as HTMLElement | null
            if (el?.closest('input, textarea, [contenteditable="true"]') || e.metaKey || e.ctrlKey || e.altKey)
                return
            if (e.key === 'j') step(1)
            else if (e.key === 'k') step(-1)
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
    })

    const lines = useMemo(() => (diff && !diff.binary ? parseUnifiedDiff(diff.diff) : []), [diff])

    return (
        <Modal open={open} onClose={() => onPath(null)} label="Changes">
            <div className="flex h-[min(804px,88vh)] w-[min(1040px,94vw)] overflow-hidden rounded-xl border border-border-strong bg-surface shadow-dialog">
                {!mobile && (
                    <nav
                        aria-label="Changed files"
                        className="flex w-280 shrink-0 flex-col border-r border-border bg-ground"
                    >
                        <header className="flex flex-col gap-2 border-b border-border px-20 py-16">
                            <h2 className="type-heading text-fg">Changes</h2>
                            <p className="type-mono-xs text-faint">
                                {changes.length === 1 ? '1 file' : `${changes.length} files`}
                            </p>
                        </header>
                        <ul className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-8">
                            {changes.map((c) => {
                                const sign = SIGN[c.kind]
                                const current = c.path === path
                                return (
                                    <li key={c.path}>
                                        <button
                                            type="button"
                                            aria-current={current}
                                            title={sign.label}
                                            onClick={() => onPath(c.path)}
                                            className={`flex w-full items-center gap-8 rounded-md border px-10 py-8 text-left hover:bg-raised ${current ? 'border-border-strong bg-raised' : 'border-transparent'}`}
                                        >
                                            <span
                                                aria-label={sign.label}
                                                className={`w-8 shrink-0 text-center type-mono-xs ${sign.tone}`}
                                            >
                                                {sign.letter}
                                            </span>
                                            <MiddleTruncate
                                                text={c.path}
                                                className="min-w-0 flex-1 type-small text-soft"
                                            />
                                            <span className="type-mono-xs text-live-text">+{c.added}</span>
                                            <span className="type-mono-xs text-failed-text">−{c.removed}</span>
                                        </button>
                                    </li>
                                )
                            })}
                        </ul>
                    </nav>
                )}
                <section aria-label="Diff" className="flex min-w-0 flex-1 flex-col">
                    <header className="flex items-center gap-12 border-b border-border px-24 py-16">
                        <div className="flex min-w-0 flex-1 flex-col gap-2">
                            <h2 className="truncate type-code-medium text-fg">{change?.path}</h2>
                            {change && (
                                <p className="type-mono-xs text-faint">
                                    {SIGN[change.kind].label.toLowerCase()} · +{change.added} −{change.removed} · file{' '}
                                    {index + 1} of {changes.length}
                                </p>
                            )}
                        </div>
                        <IconButton variant="ghost" icon="arrow-up" aria-label="Previous file" onClick={() => step(-1)} />
                        <IconButton variant="ghost" icon="arrow-down" aria-label="Next file" onClick={() => step(1)} />
                        <IconButton variant="ghost" icon="close" aria-label="Close" onClick={() => onPath(null)} />
                    </header>
                    <div className="min-h-0 flex-1 overflow-auto p-16">
                        {failed ? (
                            <EmptyState
                                kind="error"
                                icon="alert"
                                title="Could not load the diff"
                                message="Try another file or reopen this dialog."
                            />
                        ) : !diff ? (
                            <Skeleton lines={8} />
                        ) : diff.binary ? (
                            <Notice compact>Binary file; no text diff.</Notice>
                        ) : lines.length === 0 ? (
                            <Notice compact>No text changes in this file.</Notice>
                        ) : (
                            <DiffView variant="code" lines={lines} />
                        )}
                        {diff?.truncated && <Notice compact>The diff was cut off.</Notice>}
                    </div>
                    <footer className="border-t border-border px-24 py-10 type-mono-xs text-faint">
                        J / K next file · Esc close
                    </footer>
                </section>
            </div>
        </Modal>
    )
}
