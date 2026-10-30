import { Dialog, EmptyState, FileRow, FileTypeTag, IconButton, PanelHeader, TextLink } from '@vizier/design'
import { useState } from 'react'
import type { SessionFileRecord } from '../../api/frames'
import { time } from '../../lib/messages'
import { fileSize } from '../../lib/upload'
import { FilePreviewDialog } from './FilePreviewDialog'
import { clearTopicFiles, promoteTopicFile } from '../../api/endpoints/artifacts'
import { toast } from '../../stores/toasts'
import { ConfirmDialog } from '../shared/ConfirmDialog'

const SHOWN = 5

/** Files in this topic, newest first, with a dialog preview (FR-018f). */
export function SessionFilesPanel({
    files,
    agentId,
    topicId,
    canPromote,
    onClear,
}: {
    files: SessionFileRecord[]
    agentId: string
    topicId: string
    canPromote: boolean
    onClear: () => void
}) {
    const [all, setAll] = useState(false)
    const [open, setOpen] = useState<number | null>(null)
    const [promote, setPromote] = useState<string | null>(null)
    const [name, setName] = useState('')
    const [promoting, setPromoting] = useState(false)
    const [confirmClear, setConfirmClear] = useState(false)
    const [clearing, setClearing] = useState(false)
    const rows = all ? files : files.slice(0, SHOWN)

    const doPromote = async () => {
        if (!promote) return
        setPromoting(true)
        try {
            await promoteTopicFile(agentId, 'vizier-webui', topicId, {
                filename: promote,
                name: name.trim() || undefined,
            })
            toast.success(`Promoted “${name.trim() || promote}” to artifacts.`)
            setPromote(null)
            setName('')
        } catch (e) {
            toast.error("Couldn't promote the file", e instanceof Error ? e.message : undefined)
        } finally {
            setPromoting(false)
        }
    }

    const doClear = async () => {
        setClearing(true)
        try {
            const result = await clearTopicFiles(agentId, 'vizier-webui', topicId)
            toast.success(`Cleared ${result.removed} session files.`)
            setConfirmClear(false)
            onClear()
        } catch (e) {
            toast.error("Couldn't clear session files", e instanceof Error ? e.message : undefined)
        } finally {
            setClearing(false)
        }
    }

    return (
        <section aria-label="Session files" className="flex flex-col gap-8">
            <PanelHeader
                title="Session files"
                meta={`${files.length}`}
                level={3}
                action={
                    files.length > 0 ? (
                        <IconButton variant="ghost" size="sm" icon="trash" aria-label="Clear session files" title="Clear session files" onClick={() => setConfirmClear(true)} />
                    ) : undefined
                }
            />
            {files.length === 0 ? (
                <EmptyState kind="empty" icon="file" title="No files yet" message="Files the agent makes or you attach appear here." />
            ) : (
                <>
                    {rows.map((f) => (
                        <FileRow
                            key={f.id}
                            name={f.filename}
                            size={fileSize(f.size)}
                            subtitle={`${f.sender ? `${f.sender} · ` : ''}${time(f.added_at)}`}
                            onSelect={() => setOpen(files.indexOf(f))}
                            action={
                                <span className="flex items-center gap-8">
                                    {canPromote && (
                                        <TextLink
                                            onClick={() => {
                                                setPromote(f.filename)
                                                setName(f.filename)
                                            }}
                                        >
                                            Promote
                                        </TextLink>
                                    )}
                                    <FileTypeTag file={f.filename} />
                                </span>
                            }
                        />
                    ))}
                    {files.length > SHOWN && <TextLink onClick={() => setAll((a) => !a)}>{all ? 'Show fewer' : `Show all ${files.length}`}</TextLink>}
                </>
            )}
            {open !== null && files[open] && <FilePreviewDialog files={files} index={open} onIndex={setOpen} onClose={() => setOpen(null)} />}
            <Dialog
                open={promote !== null}
                title="Promote to artifacts"
                message="A permanent copy that outlives this conversation. The session file stays here."
                confirmLabel="Promote"
                onCancel={() => setPromote(null)}
                onConfirm={() => void doPromote()}
                confirmWord={undefined}
            >
                <label className="flex flex-col gap-4 type-small text-muted">
                    Artifact name
                    <input
                        aria-label="Artifact name"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        className="h-34 rounded-md border border-border-strong bg-surface px-10 type-small text-fg"
                    />
                </label>
                {promoting && <p className="type-small text-muted">Promoting…</p>}
            </Dialog>
            <ConfirmDialog
                open={confirmClear}
                title="Clear session files?"
                removes={files.map((f) => f.filename)}
                message={`This removes ${files.length} files from this conversation. Artifacts are unaffected. Older messages or memories that reference these files will no longer open them.`}
                confirmLabel="Clear session files"
                busy={clearing}
                onConfirm={() => void doClear()}
                onCancel={() => setConfirmClear(false)}
            />
        </section>
    )
}

