import { EmptyState, FileRow, FileTypeTag, PanelHeader, TextLink } from '@vizier/design'
import { useState } from 'react'
import type { SessionFileRecord } from '../../api/frames'
import { time } from '../../lib/messages'
import { fileSize } from '../../lib/upload'
import { FilePreviewDialog } from './FilePreviewDialog'

const SHOWN = 5

/** Files in this topic, newest first, with a dialog preview (FR-018f). */
export function SessionFilesPanel({ files }: { files: SessionFileRecord[] }) {
    const [all, setAll] = useState(false)
    const [open, setOpen] = useState<number | null>(null)
    const rows = all ? files : files.slice(0, SHOWN)

    return (
        <section aria-label="Session files" className="flex flex-col gap-8">
            <PanelHeader title="Session files" meta={`${files.length}`} level={3} />
            {files.length === 0 ? (
                <EmptyState kind="empty" icon="file" title="No files yet" message="Files the agent makes or you attach appear here." />
            ) : (
                <>
                    {rows.map((f) => (
                        <FileRow
                            key={f.id}
                            name={f.filename}
                            size={`${fileSize(f.size)}${f.sender ? ` · ${f.sender}` : ''} · ${time(f.added_at)}`}
                            onSelect={() => setOpen(files.indexOf(f))}
                            actions={<FileTypeTag file={f.filename} />}
                        />
                    ))}
                    {files.length > SHOWN && <TextLink onClick={() => setAll((a) => !a)}>{all ? 'Show fewer' : `Show all ${files.length}`}</TextLink>}
                </>
            )}
            {open !== null && files[open] && <FilePreviewDialog files={files} index={open} onIndex={setOpen} onClose={() => setOpen(null)} />}
        </section>
    )
}

