import { useEffect, useState } from 'react'
import { removeProject } from '../../api/endpoints/projects'
import { useProjects } from '../../stores/projects'
import { ConfirmDialog } from '../shared/ConfirmDialog'

type Props = {
    agentId: string
    project: { id: string; name: string } | null
    onClose: () => void
}

/**
 * Soft removal (FR-025): the project disappears from the sidebar, but no file and no history
 * row is deleted, and reopening the same folder restores it.
 */
export function RemoveProjectDialog({ agentId, project, onClose }: Props) {
    const forget = useProjects((s) => s.forget)
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState('')

    useEffect(() => setError(''), [project])

    return (
        <ConfirmDialog
            open={!!project}
            title="Remove this project?"
            message={
                project &&
                `“${project.name}” and its sessions leave the sidebar. Every file in the folder stays, and its history is kept: opening the same folder again restores it.`
            }
            confirmLabel="Remove project"
            busy={busy}
            error={error || undefined}
            onCancel={onClose}
            onConfirm={async () => {
                if (!project) return
                setBusy(true)
                setError('')
                try {
                    await removeProject(agentId, project.id)
                    forget(agentId, project.id)
                    onClose()
                } catch (e) {
                    // A 409 turn_active keeps the project; show the server's message in place.
                    setError(e instanceof Error ? e.message : 'The project could not be removed.')
                } finally {
                    setBusy(false)
                }
            }}
        />
    )
}
