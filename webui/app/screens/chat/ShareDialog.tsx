// Share this conversation (mockup 1), Link tab only for now. It reads the saved conversation,
// shows what a visitor would get and what was left out, and stores nothing until "Create link".
// Cancel and Preview never call the share API; the preview draws the real viewer on the
// in-memory snapshot.
import { Button, Dialog, Modal, Notice, Radio, Skeleton, SwitchRow } from '@vizier/design'
import { useCallback, useEffect, useRef, useState } from 'react'
import { ApiError } from '../../api/client'
import { transport } from '../../api/transport'
import { buildCreateRequest, createShare, sharePath, type CreatedShare } from '../../api/shares'
import { buildPreview, formatBytes, type Preview } from '../../lib/share/preview'
import { serializeConversation, ShareError, type SerializeResult } from '../../lib/share/serialize'
import { toast } from '../../stores/toasts'
import { SnapshotView } from '../../viewer/SnapshotView'

export const DESKTOP_NOTICE = 'A link needs a server others can reach. Use Offline file.'
export const COPY_WARNING = 'Anyone with the link can read this. You can revoke it later, but a copy someone already made cannot be taken back.'
export const TRAIL_HELP = 'Off: only what you and the agent said, plus images and pages.'

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`

/** "42 messages · 3 images · 1 interactive page · 612 KiB of 5 MiB". */
export function summaryLine(p: Preview): string {
    const parts = [plural(p.messages, 'message', 'messages')]
    if (p.images) parts.push(plural(p.images, 'image', 'images'))
    if (p.pages) parts.push(plural(p.pages, 'interactive page', 'interactive pages'))
    if (p.questions) parts.push(plural(p.questions, 'question', 'questions'))
    if (p.includeTrail) parts.push(plural(p.trailEntries, 'activity step', 'activity steps'))
    parts.push(`${formatBytes(p.sizeBytes)} of ${formatBytes(p.limitBytes)}`)
    return parts.join(' · ')
}

/** What was left out on its own, with counts; the activity trail counts too while it is off. */
export function leftOutLines(p: Preview): string[] {
    const lines = p.removed.map((r) => r.label)
    if (!p.includeTrail && p.omittedTrail > 0) lines.push(plural(p.omittedTrail, 'activity step (tool call or result)', 'activity steps (tool calls and results)'))
    return lines
}

/** Images and files that were replaced by a placeholder. */
export function warningLines(p: Preview): string[] {
    return p.placeholders.map((x) => (x.reason === 'oversized' ? `${x.label} was replaced by a placeholder (too large)` : `${x.label} was replaced by a placeholder`))
}

export const linkFor = (origin: string, created: Pick<CreatedShare, 'share_id'>) => `${origin}${sharePath(created.share_id)}`

/** The sentence for a failed create. */
export function createFailure(e: unknown): string {
    if (e instanceof ShareError) return e.message
    if (e instanceof ApiError) {
        if (e.status === 429) return 'Too many shares just now. Try again in a little while.'
        if (e.status === 403) return 'You are not allowed to share this conversation.'
        return e.message || 'The link could not be created.'
    }
    return 'The link could not be created.'
}

/** The one call that stores something: builds the request from what was previewed and sends it. */
export async function createLink(choice: { agentId: string; topicId: string; title: string }, result: SerializeResult, origin: string): Promise<string> {
    const made = await createShare(buildCreateRequest({ ...choice, audience: 'public' }, result))
    return linkFor(origin, made)
}

type Props = {
    open: boolean
    onClose: () => void
    agentId: string
    topicId: string
    title: string
    agentName: string
}

export function ShareDialog({ open, onClose, agentId, topicId, title, agentName }: Props) {
    const desktop = transport.authMode === 'desktop'
    const [includeTrail, setIncludeTrail] = useState(false)
    const [result, setResult] = useState<SerializeResult | null>(null)
    const [loadError, setLoadError] = useState<string | null>(null)
    const [loading, setLoading] = useState(false)
    const [creating, setCreating] = useState(false)
    const [createError, setCreateError] = useState<string | null>(null)
    const [created, setCreated] = useState<string | null>(null)
    const [previewing, setPreviewing] = useState(false)
    const run = useRef(0)

    // Start clean each time it opens.
    useEffect(() => {
        if (open) {
            setIncludeTrail(false)
            setCreated(null)
            setCreateError(null)
            setPreviewing(false)
        }
    }, [open])

    // Read the conversation (the trail switch changes what is in it). Reads only.
    useEffect(() => {
        if (!open || desktop) return
        const mine = ++run.current
        setLoading(true)
        setLoadError(null)
        serializeConversation({ agentId, topicId, agentName, title, includeTrail })
            .then((r) => mine === run.current && setResult(r))
            .catch((e) => mine === run.current && (setResult(null), setLoadError(e instanceof Error ? e.message : 'The conversation could not be read.')))
            .finally(() => mine === run.current && setLoading(false))
        return () => {
            run.current++
        }
    }, [open, desktop, agentId, topicId, agentName, title, includeTrail])

    const preview = result ? buildPreview(result) : null

    const create = useCallback(async () => {
        if (!result || !preview?.canShare) return
        setCreating(true)
        setCreateError(null)
        try {
            const link = await createLink({ agentId, topicId, title }, result, window.location.origin)
            setCreated(link)
            void navigator.clipboard?.writeText(link).then(() => toast.success('Link copied'), () => undefined)
        } catch (e) {
            setCreateError(createFailure(e))
        } finally {
            setCreating(false)
        }
    }, [result, preview, agentId, topicId, title])

    const copy = () => {
        if (!created) return
        navigator.clipboard?.writeText(created).then(
            () => toast.success('Link copied'),
            () => toast.error("Couldn't copy the link", 'Select it and copy by hand.')
        )
    }

    return (
        <>
            <Dialog
                open={open}
                size="lg"
                title="Share this conversation"
                confirmLabel={created || desktop ? '' : 'Create link'}
                confirmLoading={creating}
                confirmDisabled={!preview?.canShare || loading}
                onConfirm={() => void create()}
                onCancel={onClose}
            >
                <div role="tablist" aria-label="How to share" className="flex gap-8">
                    <button type="button" role="tab" aria-selected="true" className="type-ui-medium text-fg underline underline-offset-4">Link</button>
                </div>

                {desktop ? (
                    <Notice tone="info" title={DESKTOP_NOTICE} />
                ) : created ? (
                    <div className="flex flex-col gap-12">
                        <p className="type-ui text-muted">The link is ready and copied.</p>
                        <div className="flex items-center gap-8">
                            <input readOnly aria-label="Link" value={created} onFocus={(e) => e.currentTarget.select()} className="min-w-0 flex-1 rounded-md border border-border-strong bg-raised px-10 py-6 type-mono-xs text-fg" />
                            <Button onClick={copy}>Copy</Button>
                            <Button variant="primary" onClick={onClose}>Done</Button>
                        </div>
                        <p className="type-small text-muted">{COPY_WARNING}</p>
                    </div>
                ) : (
                    <div className="flex flex-col gap-16">
                        <fieldset className="flex flex-col gap-6">
                            <legend className="type-ui-medium text-fg">Who can open it</legend>
                            <Radio name="audience" label="Specific people (coming soon)" disabled checked={false} readOnly />
                            <Radio name="audience" label="Anyone signed in on this instance (coming soon)" disabled checked={false} readOnly />
                            <Radio name="audience" label="Anyone with the link (public)" checked readOnly />
                        </fieldset>

                        <SwitchRow label="Include the agent's activity trail (tool calls and results)" helper={TRAIL_HELP} checked={includeTrail} onCheckedChange={setIncludeTrail} />

                        <section className="flex flex-col gap-6" aria-label="What will be shared">
                            <h3 className="type-ui-medium text-fg">What will be shared</h3>
                            {loading && !preview && <Skeleton variant="lines" lines={2} />}
                            {loadError && <Notice tone="failed" title="The conversation could not be read">{loadError}</Notice>}
                            {preview && (
                                <>
                                    <p className="type-ui text-fg">{summaryLine(preview)}</p>
                                    {leftOutLines(preview).length > 0 && (
                                        <div>
                                            <p className="type-small text-muted">Left out automatically:</p>
                                            <ul className="list-disc pl-20 type-small text-muted">
                                                {leftOutLines(preview).map((l) => <li key={l}>{l}</li>)}
                                            </ul>
                                        </div>
                                    )}
                                    {warningLines(preview).map((w) => <p key={w} role="alert" className="type-small text-retrying">⚠ {w}</p>)}
                                    {preview.notes.map((n) => <p key={n} className="type-small text-muted">{n}</p>)}
                                    {preview.refusal && (
                                        <Notice tone="failed" title={preview.refusal.message}>
                                            {preview.refusal.options.length > 0 && <ul className="list-disc pl-20">{preview.refusal.options.map((o) => <li key={o}>{o}</li>)}</ul>}
                                        </Notice>
                                    )}
                                    <div><Button onClick={() => setPreviewing(true)} disabled={!result}>Preview what a visitor sees</Button></div>
                                </>
                            )}
                        </section>

                        {createError && <Notice tone="failed" title={createError} />}
                        <p className="type-small text-muted">{COPY_WARNING}</p>
                    </div>
                )}
            </Dialog>
            {result && (
                <Modal open={previewing} onClose={() => setPreviewing(false)} label="Preview of the shared conversation">
                    <div className="max-h-dvh overflow-auto bg-ground">
                        <div className="flex justify-end p-8"><Button onClick={() => setPreviewing(false)}>Close preview</Button></div>
                        <SnapshotView snapshot={result.snapshot} banner="Preview: nothing has been shared yet" />
                    </div>
                </Modal>
            )}
        </>
    )
}
