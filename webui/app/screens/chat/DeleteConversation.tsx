import { useState } from 'react'
import { deleteTopic } from '../../api/endpoints/chat'
import { toast } from '../../stores/toasts'
import { ConfirmDialog } from '../shared/ConfirmDialog'

export type DeleteTarget = { topic: string; title: string }

type Props = {
    agentId: string
    /** The conversation to delete; the dialog is open while there is one. */
    target: DeleteTarget | null
    onClose: () => void
    /** Called after the server has deleted it. */
    onDeleted: (topic: string) => void
}

/** Asks before deleting a WebUI conversation, then deletes it. The server refuses while a turn is running in it. */
export function DeleteConversation({ agentId, target, onClose, onDeleted }: Props) {
    const [busy, setBusy] = useState(false)
    return (
        <ConfirmDialog
            open={!!target}
            title="Delete this conversation?"
            message={target && `“${target.title}” and its messages and files will be removed.`}
            confirmLabel="Delete"
            busy={busy}
            onCancel={onClose}
            onConfirm={async () => {
                if (!target) return
                setBusy(true)
                try {
                    await deleteTopic(agentId, target.topic)
                    onClose()
                    onDeleted(target.topic)
                } catch (e) {
                    toast.error("Couldn't delete the conversation", e instanceof Error ? e.message : undefined)
                    onClose()
                } finally {
                    setBusy(false)
                }
            }}
        />
    )
}
