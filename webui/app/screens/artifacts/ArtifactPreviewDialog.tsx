import { Dialog } from '@vizier/design'
import { useEffect, useState } from 'react'
import type { ArtifactItem } from '../../api/endpoints/artifacts'
import { artifactContentUrl } from '../../api/endpoints/artifacts'
import { canPreview, previewNote } from '../../lib/artifacts'

/** The preview body for one artifact: image, PDF, CSV/text/markdown, or a download note. */
export function ArtifactPreviewDialog({
    artifact,
    open,
    onClose,
}: {
    artifact: ArtifactItem | null
    open: boolean
    onClose: () => void
}) {
    const [text, setText] = useState<string | null>(null)
    const [error, setError] = useState<string | null>(null)
    const url = artifact ? artifactContentUrl(artifact.agent_id, artifact.id) : ''

    useEffect(() => {
        setText(null)
        setError(null)
        if (!artifact || !open) return
        if (!['text', 'markdown', 'csv'].includes(artifact.kind)) return
        let alive = true
        fetch(url)
            .then(async (res) => {
                if (!res.ok) throw new Error(`HTTP ${res.status}`)
                return res.text()
            })
            .then((body) => alive && setText(body))
            .catch((e) => alive && setError(e instanceof Error ? e.message : 'Could not load'))
        return () => {
            alive = false
        }
    }, [artifact, open, url])

    if (!artifact) return null
    return (
        <Dialog
            open={open}
            title={artifact.name}
            confirmLabel=""
            onConfirm={() => {}}
            onCancel={onClose}
            size="lg"
        >
            {!artifact.available ? (
                <p className="type-ui-para text-failed" role="status">
                    {artifact.name} — artifact unavailable
                </p>
            ) : !canPreview(artifact.kind) ? (
                <p className="type-ui-para text-muted">{previewNote(artifact.kind)}</p>
            ) : artifact.kind === 'image' ? (
                <img src={url} alt={artifact.name} className="max-h-480 w-full rounded-lg object-contain" />
            ) : artifact.kind === 'pdf' ? (
                <iframe title={artifact.name} src={url} className="h-480 w-full rounded-lg border border-border" />
            ) : error ? (
                <p className="type-ui-para text-failed" role="alert">
                    Couldn't load the preview: {error}
                </p>
            ) : text === null ? (
                <p className="type-ui-para text-muted">Loading…</p>
            ) : (
                <pre className="max-h-480 overflow-auto rounded-lg bg-raised p-12 type-code text-fg">{text}</pre>
            )}
            <div className="flex justify-end gap-10 pt-4">
                <a
                    className="type-ui-para text-link underline"
                    href={url}
                    target="_blank"
                    rel="noreferrer"
                    download={artifact.name}
                >
                    Download
                </a>
            </div>
        </Dialog>
    )
}
