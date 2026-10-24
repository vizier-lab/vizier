import { Button, Dialog, Notice } from '@vizier/design'
import type { ReactNode } from 'react'

type Props = {
    open: boolean
    title: ReactNode
    /** What will be removed, one line each. */
    removes?: string[]
    message?: ReactNode
    confirmLabel: ReactNode
    /** Asks for this word (an agent ID) before the confirm enables. */
    confirmWord?: string
    /** The action is running: the confirm shows a spinner and cannot be pressed again. */
    busy?: boolean
    /** The action failed: shown above the actions and the dialog stays open. */
    error?: ReactNode
    onConfirm: () => void
    onCancel: () => void
}

/** A danger confirmation that lists what goes (States page). */
export function ConfirmDialog({ open, title, removes, message, confirmLabel, confirmWord, busy, error, onConfirm, onCancel }: Props) {
    const messageNode = (
        <>
            {message}
            {removes && removes.length > 0 && (
                <ul className="mt-8 list-disc pl-20">
                    {removes.map((r) => (
                        <li key={r}>{r}</li>
                    ))}
                </ul>
            )}
        </>
    )
    // A dialog that reports its own progress keeps its actions, so a failure can be
    // shown in place instead of closing and losing what happened.
    if (busy !== undefined || error !== undefined) {
        return (
            <Dialog open={open} tone="danger" title={title} message={messageNode} confirmLabel="" onConfirm={() => {}} onCancel={onCancel}>
                <div className="flex flex-col gap-16">
                    {error && <Notice tone="failed" title="It didn’t work">{error}</Notice>}
                    <div className="flex justify-end gap-10">
                        <Button onClick={onCancel} disabled={busy}>Cancel</Button>
                        <Button variant="destructive" loading={busy} onClick={onConfirm}>
                            {confirmLabel}
                        </Button>
                    </div>
                </div>
            </Dialog>
        )
    }
    return (
        <Dialog
            open={open}
            tone="danger"
            title={title}
            message={messageNode}
            confirmWord={confirmWord}
            confirmLabel={confirmLabel}
            onConfirm={onConfirm}
            onCancel={onCancel}
        />
    )
}
