import { Button, Dialog, Notice, TextField } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { createProjectSession, openProject } from '../../api/endpoints/projects'
import { transport } from '../../api/transport'
import { useProjects } from '../../stores/projects'
import { TrustDisclosure } from './TrustDisclosure'

type Props = {
    agentId: string
    open: boolean
    onClose: () => void
}

/**
 * Desktop-only project creation (FR-001, FR-002): choose a folder, accept the trust disclosure,
 * then open it and start its first session. A browser never sees this component's confirm and
 * the server refuses a network principal anyway.
 */
export function OpenProjectDialog({ agentId, open, onClose }: Props) {
    const navigate = useNavigate()
    const refresh = useProjects((s) => s.refresh)
    const insertSession = useProjects((s) => s.insertSession)
    const desktop = transport.authMode === 'desktop'
    const [folder, setFolder] = useState('')
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState('')

    useEffect(() => {
        if (!open) return
        setFolder('')
        setError('')
        setBusy(false)
    }, [open])

    const choose = async () => {
        setError('')
        try {
            const chosen = await transport.pickFolder()
            if (chosen) setFolder(chosen)
        } catch (e) {
            setError(e instanceof Error ? e.message : 'The folder picker could not open.')
        }
    }

    const confirm = async () => {
        if (!folder) return
        setBusy(true)
        setError('')
        try {
            const project = await openProject(agentId, folder)
            const session = await createProjectSession(agentId, project.id)
            insertSession(agentId, project.id, {
                topic: session.topic,
                title: session.title,
                is_thinking: false,
                last_activity: null,
            })
            await refresh(agentId)
            onClose()
            navigate(`/${agentId}/projects/${project.id}/chat/${session.topic}`)
        } catch (e) {
            // Keep the dialog open with the server's message (invalid or workspace folder, 403).
            setError(e instanceof Error ? e.message : 'The project could not be opened.')
        } finally {
            setBusy(false)
        }
    }

    return (
        <Dialog
            open={open}
            title="Open a Project"
            message="Choose a folder on this machine for the agent to work in."
            confirmLabel=""
            size="lg"
            onConfirm={() => {}}
            onCancel={onClose}
        >
            <div className="flex flex-col gap-14">
                {!desktop ? (
                    <Notice tone="info" icon="lock" title="The browser cannot open a project">
                        Projects are created in the desktop app, where a local folder can be
                        chosen. Existing projects stay available here.
                    </Notice>
                ) : (
                    <>
                        <div className="flex items-end gap-10">
                            <TextField
                                label="Folder"
                                mono
                                value={folder}
                                placeholder="No folder chosen"
                                readOnly
                                className="min-w-0 flex-1"
                                onChange={() => {}}
                            />
                            <Button onClick={() => void choose()} disabled={busy}>
                                Choose…
                            </Button>
                        </div>
                        {folder && <TrustDisclosure folder={folder} />}
                        {error && (
                            <Notice tone="failed" title="It didn’t work">
                                {error}
                            </Notice>
                        )}
                        <div className="flex justify-end gap-10">
                            <Button onClick={onClose} disabled={busy}>
                                Cancel
                            </Button>
                            <Button
                                variant="primary"
                                loading={busy}
                                disabled={!folder}
                                onClick={() => void confirm()}
                            >
                                Open project
                            </Button>
                        </div>
                    </>
                )}
            </div>
        </Dialog>
    )
}
