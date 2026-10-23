import { Dialog } from '@vizier/design'
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
    onConfirm: () => void
    onCancel: () => void
}

/** A danger confirmation that lists what goes (States page). */
export function ConfirmDialog({ open, title, removes, message, confirmLabel, confirmWord, onConfirm, onCancel }: Props) {
    return (
        <Dialog
            open={open}
            tone="danger"
            title={title}
            message={
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
            }
            confirmWord={confirmWord}
            confirmLabel={confirmLabel}
            onConfirm={onConfirm}
            onCancel={onCancel}
        />
    )
}
