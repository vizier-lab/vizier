import type { ArtifactEvent, ArtifactItem, ArtifactKind } from '../api/endpoints/artifacts'

export const ARTIFACT_KINDS: { value: ArtifactKind | 'all'; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'image', label: 'Images' },
    { value: 'pdf', label: 'PDFs' },
    { value: 'text', label: 'Text' },
    { value: 'markdown', label: 'Markdown' },
    { value: 'csv', label: 'CSV' },
    { value: 'audio', label: 'Audio' },
    { value: 'video', label: 'Video' },
    { value: 'other', label: 'Other' },
]

/** Human label for a kind. */
export function kindLabel(kind: ArtifactKind): string {
    return ARTIFACT_KINDS.find((k) => k.value === kind)?.label ?? 'Other'
}

/** A stable text/icon warning used instead of colour alone. */
export function unavailableLabel(artifact: Pick<ArtifactItem, 'name'>): string {
    return `${artifact.name} — artifact unavailable`
}

/** Whether the artifact can be previewed in the gallery. */
export function canPreview(kind: ArtifactKind): boolean {
    return ['image', 'pdf', 'text', 'markdown', 'csv'].includes(kind)
}

export function previewNote(kind: ArtifactKind): string {
    return canPreview(kind) ? '' : 'No preview for this type. Download it to open it.'
}

export function actionLabel(event: Pick<ArtifactEvent, 'action'>): string {
    switch (event.action) {
        case 'promoted':
            return 'Promoted'
        case 'replaced':
            return 'Replaced'
        case 'deleted':
            return 'Deleted'
        default:
            return 'Changed'
    }
}

export function referenceLabel(reference: { kind: string; label: string }): string {
    return reference.kind === 'memory' ? `Memory · ${reference.label}` : `Message · ${reference.label}`
}

/** `1 file`, `4 files`; `0 B`, `1.2 KB`. */
export function plural(n: number, one: string, many = `${one}s`): string {
    return `${n} ${n === 1 ? one : many}`
}

export function bytesLabel(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}
