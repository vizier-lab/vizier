import { Button, Dialog, KeyValueList } from '@vizier/design'
import type { ArtifactDetail as Detail } from '../../api/endpoints/artifacts'
import { artifactContentUrl } from '../../api/endpoints/artifacts'
import { actionLabel, bytesLabel, kindLabel, referenceLabel } from '../../lib/artifacts'
import { relativeTime } from '../../lib/format'

/** One artifact: metadata, origin, provenance and where it is used. */
export function ArtifactDetail({
    detail,
    open,
    canManage,
    onClose,
    onDelete,
    onPreview,
}: {
    detail: Detail | null
    open: boolean
    canManage: boolean
    onClose: () => void
    onDelete: () => void
    onPreview: () => void
}) {
    if (!detail) return null
    const { artifact, origin, references, events } = detail
    const url = artifactContentUrl(artifact.agent_id, artifact.id)
    return (
        <Dialog
            open={open}
            title={artifact.name}
            confirmLabel=""
            onConfirm={() => {}}
            onCancel={onClose}
            size="lg"
        >
            <div className="flex flex-col gap-16">
                {!detail.available && (
                    <p role="status" className="type-ui-para text-failed-text">
                        {artifact.name} — artifact unavailable
                    </p>
                )}
                <KeyValueList
                    items={[
                        { label: 'Kind', value: `${kindLabel(artifact.kind)} · ${artifact.mime_type}` },
                        { label: 'Size', value: bytesLabel(artifact.size) },
                        { label: 'Version', value: artifact.version },
                        { label: 'Created', value: relativeTime(artifact.created_at) },
                        { label: 'Changed', value: relativeTime(artifact.updated_at) },
                        { label: 'Origin', value: `${origin.channel ?? 'unknown'}${origin.source_filename ? ` · ${origin.source_filename}` : ''}` },
                    ]}
                />

                <section aria-labelledby="artifact-provenance">
                    <h3 id="artifact-provenance" className="type-ui-strong text-fg">Provenance</h3>
                    <ul className="mt-6 flex flex-col gap-4">
                        {events.map((event) => (
                            <li key={event.id} className="type-small text-muted">
                                <span className="text-fg">{actionLabel(event)}</span> by {event.actor_name} ·{' '}
                                {relativeTime(event.created_at)}
                                {event.source_filename ? ` · ${event.source_filename}` : ''}
                            </li>
                        ))}
                        {events.length === 0 && <li className="type-small text-muted">No changes recorded.</li>}
                    </ul>
                </section>

                <section aria-labelledby="artifact-references">
                    <h3 id="artifact-references" className="type-ui-strong text-fg">Used by</h3>
                    {references.length === 0 ? (
                        <p className="mt-6 type-small text-muted">Not attached anywhere yet.</p>
                    ) : (
                        <ul className="mt-6 flex flex-col gap-4">
                            {references.map((reference) => (
                                <li key={`${reference.kind}-${reference.label}-${reference.created_at}`}>
                                    {reference.href ? (
                                        <a className="type-small text-link underline" href={reference.href}>
                                            {referenceLabel(reference)}
                                        </a>
                                    ) : (
                                        <span className="type-small text-muted">{referenceLabel(reference)}</span>
                                    )}
                                </li>
                            ))}
                        </ul>
                    )}
                </section>

                <div className="flex flex-wrap justify-end gap-10 pt-4">
                    <Button onClick={onPreview}>Preview</Button>
                    <a className="type-ui-para self-center text-link underline" href={url} target="_blank" rel="noreferrer" download={artifact.name}>
                        Download
                    </a>
                    {canManage && (
                        <Button variant="destructive" onClick={onDelete}>
                            Delete
                        </Button>
                    )}
                </div>
            </div>
        </Dialog>
    )
}
