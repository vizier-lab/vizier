// Share this conversation (mockup 1), Link tab only for now. It reads the saved conversation,
// shows what a visitor would get and what was left out, and stores nothing until "Create link".
// Cancel and Preview never call the share API; the preview draws the real viewer on the
// in-memory snapshot.
import { Button, Checkbox, Dialog, Modal, Notice, Radio, Skeleton, SwitchRow } from '@vizier/design'
import { useCallback, useEffect, useRef, useState } from 'react'
import { ApiError } from '../../api/client'
import { transport } from '../../api/transport'
import { buildCreateRequest, createShare, searchPeople, sharePath, type Audience, type CreatedShare, type PersonMatch } from '../../api/shares'
import { singleFlight } from '../../lib/share/singleFlight'
import { exportOfflineSnapshot } from '../../lib/share/export'
import { SHARE_LIMITS } from '../../lib/share/limits'
import { buildPreview, formatBytes, type Preview } from '../../lib/share/preview'
import { serializeConversation, ShareError, type SerializeResult } from '../../lib/share/serialize'
import { toast } from '../../stores/toasts'
import { SnapshotView } from '../../viewer/SnapshotView'

export const DESKTOP_NOTICE = 'A link needs a server others can reach. Use Offline file.'
export const OFFLINE_NOTICE = 'Nothing is stored on the server. The offline file contains everything needed to view the conversation interactively in any web browser without an internet connection.'
/** FR-020: removal is automatic and best effort; the person reads before sharing. */
export const REVIEW_WARNING = 'Automatic removal cannot recognise everything sensitive that you or the agent may have written. Read the preview before you share: you are responsible for what leaves.'
export const OFFLINE_WARNING = 'Once you send this file to someone, it cannot be revoked or limited to an audience.'
export const COPY_WARNING = 'Anyone with the link can read this. You can revoke it later, but a copy someone already made cannot be taken back.'
export const PUBLIC_CONFIRM = 'I understand that anyone who has this link can read the conversation without signing in.'
export const CHOOSE_AUDIENCE = 'Choose who can open the link. Nothing is chosen for you.'
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

export const SECRET_DETECTED = 'secret_detected'
export const SECRET_REFUSED = 'The server found what looks like a password, key or token in this conversation, so nothing was shared. Remove it from the conversation (or leave that part out) and try again.'

/** The sentence for a failed create. */
export function createFailure(e: unknown): string {
    if (e instanceof ShareError) return e.message
    if (e instanceof ApiError) {
        // The server's last-resort backstop (FR-051). It names the kind of secret, never the text.
        if (e.status === 422 && e.code === SECRET_DETECTED) return SECRET_REFUSED
        if (e.status === 429) return 'Too many shares just now. Try again in a little while.'
        if (e.status === 403) return 'You are not allowed to share this conversation.'
        if (e.status === 413) return 'This conversation is too large to share. Nothing was shared.'
        // A bare "Service Unavailable" says nothing; say what happened and that nothing was stored.
        if (e.status >= 500) return 'The server could not create the link right now. Nothing was shared; try again in a moment.'
        return e.message || 'The link could not be created.'
    }
    return 'The link could not be created.'
}

/** The one call that stores something: builds the request from what was previewed and sends it. */
export async function createLink(
    choice: { agentId: string; topicId: string; title: string; audience: Audience; people?: string[] },
    result: SerializeResult,
    origin: string
): Promise<string> {
    // No default audience: a public link is never what happens when nothing was chosen (FR-011).
    if (!choice.audience) throw new ShareError('audience', 'Choose who can open the link.')
    const made = await createShare(buildCreateRequest(choice, result))
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
    const [tab, setTab] = useState<'link' | 'offline'>(desktop ? 'offline' : 'link')
    const [includeTrail, setIncludeTrail] = useState(false)
    const [audience, setAudience] = useState<Audience | null>(null)
    const [people, setPeople] = useState<PersonMatch[]>([])
    const [peopleQuery, setPeopleQuery] = useState('')
    const [peopleMatches, setPeopleMatches] = useState<PersonMatch[]>([])
    const [searchingPeople, setSearchingPeople] = useState(false)
    const [peopleError, setPeopleError] = useState<string | null>(null)
    // FR: an "anyone with the link" share needs a second, explicit confirmation.
    const [confirmedPublic, setConfirmedPublic] = useState(false)
    const [result, setResult] = useState<SerializeResult | null>(null)
    const [loadError, setLoadError] = useState<string | null>(null)
    const [loading, setLoading] = useState(false)
    const [creating, setCreating] = useState(false)
    const [createError, setCreateError] = useState<string | null>(null)
    const [exporting, setExporting] = useState(false)
    const [exportError, setExportError] = useState<string | null>(null)
    const [created, setCreated] = useState<string | null>(null)
    const [previewing, setPreviewing] = useState(false)
    const run = useRef(0)
    // FR-050: confirming twice (double click, key repeat) must create one share.
    const createOnce = useRef(singleFlight<void>())

    // Start clean each time it opens.
    useEffect(() => {
        if (open) {
            setTab(desktop ? 'offline' : 'link')
            setIncludeTrail(false)
            setAudience(null)
            setPeople([])
            setPeopleQuery('')
            setPeopleMatches([])
            setPeopleError(null)
            setConfirmedPublic(false)
            setCreated(null)
            setCreateError(null)
            setExporting(false)
            setExportError(null)
            setPreviewing(false)
        }
    }, [open, desktop])

    // Search people by query prefix when audience is 'specific'
    useEffect(() => {
        if (audience !== 'specific') return
        const q = peopleQuery.trim()
        if (q.length < 2) {
            setPeopleMatches([])
            setSearchingPeople(false)
            return
        }
        const ctl = new AbortController()
        setSearchingPeople(true)
        const t = setTimeout(() => {
            searchPeople(q, ctl.signal)
                .then((matches) => {
                    if (!ctl.signal.aborted) {
                        setPeopleMatches(matches.slice(0, SHARE_LIMITS.peopleResults).filter((m) => !people.some((p) => p.user_id === m.user_id)))
                        setPeopleError(null)
                    }
                })
                .catch((err) => {
                    if (!ctl.signal.aborted) {
                        setPeopleError(err instanceof Error ? err.message : 'Could not search people')
                    }
                })
                .finally(() => {
                    if (!ctl.signal.aborted) setSearchingPeople(false)
                })
        }, 200)
        return () => {
            clearTimeout(t)
            ctl.abort()
        }
    }, [audience, peopleQuery, people])

    const addPerson = (p: PersonMatch) => {
        if (people.length >= SHARE_LIMITS.people) return
        if (!people.some((existing) => existing.user_id === p.user_id)) {
            setPeople([...people, p])
        }
        setPeopleQuery('')
        setPeopleMatches([])
    }

    const removePerson = (userId: string) => {
        setPeople(people.filter((p) => p.user_id !== userId))
    }

    // Read the conversation (the trail switch changes what is in it). Reads only.
    useEffect(() => {
        if (!open) return
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
    }, [open, agentId, topicId, agentName, title, includeTrail])

    const preview = result ? buildPreview(result) : null

    const doExport = useCallback(async () => {
        if (!result?.snapshot) return
        setExporting(true)
        setExportError(null)
        try {
            const { filename } = await exportOfflineSnapshot(result.snapshot)
            toast.success(`Exported ${filename}`)
            onClose()
        } catch (e) {
            setExportError(e instanceof Error ? e.message : 'Could not export file.')
        } finally {
            setExporting(false)
        }
    }, [result, onClose])

    const doCreate = useCallback(
        async (chosen: Audience) => {
            if (!result) return
            setCreating(true)
            setCreateError(null)
            try {
                const link = await createLink(
                    {
                        agentId,
                        topicId,
                        title,
                        audience: chosen,
                        people: chosen === 'specific' ? people.map((p) => p.user_id) : undefined,
                    },
                    result,
                    window.location.origin
                )
                setCreated(link)
                void navigator.clipboard?.writeText(link).then(() => toast.success('Link copied'), () => undefined)
            } catch (e) {
                setCreateError(createFailure(e))
            } finally {
                setCreating(false)
            }
        },
        [result, agentId, topicId, title, people]
    )

    const create = useCallback(async () => {
        if (!result || !preview?.canShare || !audience) return
        if (audience === 'specific' && people.length === 0) return
        if (audience === 'public' && !confirmedPublic) return
        await createOnce.current(() => doCreate(audience))
    }, [result, preview, audience, people, confirmedPublic, doCreate])

    const copy = () => {
        if (!created) return
        navigator.clipboard?.writeText(created).then(
            () => toast.success('Link copied'),
            () => toast.error("Couldn't copy the link", 'Select it and copy by hand.')
        )
    }

    const audienceLabel =
        audience === 'public'
            ? 'Anyone with the link'
            : audience === 'signed_in'
            ? 'Anyone signed in on this instance'
            : audience === null
            ? 'Not chosen'
            : `Specific people (${plural(people.length, 'person', 'people')})`

    return (
        <>
            <Dialog
                open={open}
                size="lg"
                title="Share this conversation"
                confirmLabel={
                    tab === 'link'
                        ? created || desktop ? '' : 'Create link'
                        : exporting ? 'Exporting…' : 'Export file'
                }
                confirmLoading={tab === 'link' ? creating : exporting}
                confirmDisabled={
                    tab === 'link'
                        ? !preview?.canShare || loading || audience === null || (audience === 'specific' && people.length === 0) || (audience === 'public' && !confirmedPublic)
                        : !preview?.canShare || loading || exporting
                }
                onConfirm={() => {
                    if (tab === 'link') void create()
                    else void doExport()
                }}
                onCancel={onClose}
            >
                <div role="tablist" aria-label="How to share" className="flex gap-16 border-b border-border pb-8">
                    <button
                        type="button"
                        role="tab"
                        aria-selected={tab === 'link'}
                        onClick={() => setTab('link')}
                        className={tab === 'link' ? 'type-ui-medium text-fg underline underline-offset-8 font-semibold' : 'type-ui-medium text-muted hover:text-fg'}
                    >
                        Link
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected={tab === 'offline'}
                        onClick={() => setTab('offline')}
                        className={tab === 'offline' ? 'type-ui-medium text-fg underline underline-offset-8 font-semibold' : 'type-ui-medium text-muted hover:text-fg'}
                    >
                        Offline file
                    </button>
                </div>

                {tab === 'link' ? (
                    desktop ? (
                        <Notice tone="info" title={DESKTOP_NOTICE} />
                    ) : created ? (
                        <div className="flex flex-col gap-12">
                            <p className="type-ui text-muted">The link is ready and copied.</p>
                            <div className="flex items-center gap-8">
                                <input readOnly aria-label="Link" value={created} onFocus={(e) => e.currentTarget.select()} className="min-w-0 flex-1 rounded-md border border-border-strong bg-raised px-10 py-6 type-mono-xs text-fg" />
                                <Button onClick={copy}>Copy</Button>
                                <Button variant="primary" onClick={onClose}>Done</Button>
                            </div>
                            <p className="type-small text-muted">Who can open it: <span className="text-fg">{audienceLabel}</span> (cannot be changed after creating).</p>
                            <p className="type-small text-muted">{COPY_WARNING}</p>
                        </div>
                    ) : (
                        <div className="flex flex-col gap-16">
                            <fieldset className="flex flex-col gap-6">
                                <legend className="type-ui-medium text-fg">Who can open it</legend>
                                {audience === null && <p className="type-small text-muted">{CHOOSE_AUDIENCE}</p>}
                                <Radio
                                    name="audience"
                                    label="Specific people"
                                    checked={audience === 'specific'}
                                    onChange={() => setAudience('specific')}
                                />
                                {audience === 'specific' && (
                                    <div className="flex flex-col gap-8 pl-24">
                                        <div className="relative">
                                            <input
                                                type="search"
                                                aria-label="Add people by username"
                                                placeholder="Search username (at least 2 characters)…"
                                                value={peopleQuery}
                                                onChange={(e) => setPeopleQuery(e.target.value)}
                                                className="w-full rounded-md border border-border-strong bg-raised px-10 py-6 type-ui-xs text-fg placeholder:text-muted focus:border-ink"
                                            />
                                            <span role="status" aria-live="polite" className="sr-only">{searchingPeople ? 'Searching…' : peopleQuery.trim().length >= 2 ? plural(peopleMatches.length, 'match', 'matches') : ''}</span>
                                            {searchingPeople && <span aria-hidden="true" className="absolute right-8 top-8 type-ui-xs text-muted">Searching…</span>}
                                            {peopleMatches.length > 0 && (
                                                <ul
                                                    aria-label="Matching users"
                                                    className="absolute z-10 mt-4 max-h-48 w-full overflow-auto rounded-md border border-border-strong bg-raised py-4 shadow-lg"
                                                >
                                                    {peopleMatches.map((match) => (
                                                        <li key={match.user_id}>
                                                            <button
                                                                type="button"
                                                                onClick={() => addPerson(match)}
                                                                className="w-full px-10 py-6 text-left type-ui-xs text-fg hover:bg-hover"
                                                            >
                                                                {match.username}
                                                            </button>
                                                        </li>
                                                    ))}
                                                </ul>
                                            )}
                                        </div>
                                        {peopleError && <p role="alert" className="type-small text-failed-text">{peopleError}</p>}
                                        {people.length > 0 ? (
                                            <div role="group" className="flex flex-wrap gap-6" aria-label="Selected people">
                                                {people.map((p) => (
                                                    <span
                                                        key={p.user_id}
                                                        className="inline-flex items-center gap-4 rounded-xs border border-border-strong bg-raised px-6 py-2 type-ui-xs text-fg"
                                                    >
                                                        {p.username}
                                                        <button
                                                            type="button"
                                                            aria-label={`Remove ${p.username}`}
                                                            onClick={() => removePerson(p.user_id)}
                                                            className="type-ui-xs text-muted hover:text-fg"
                                                        >
                                                            ×
                                                        </button>
                                                    </span>
                                                ))}
                                            </div>
                                        ) : (
                                            <p className="type-small text-muted">Name at least one person to share with.</p>
                                        )}
                                    </div>
                                )}
                                <Radio
                                    name="audience"
                                    label="Anyone signed in on this instance"
                                    checked={audience === 'signed_in'}
                                    onChange={() => setAudience('signed_in')}
                                />
                                <Radio
                                    name="audience"
                                    label="Anyone with the link (public)"
                                    checked={audience === 'public'}
                                    onChange={() => setAudience('public')}
                                />
                                {audience === 'public' && (
                                    <div className="pl-24">
                                        <Checkbox label={PUBLIC_CONFIRM} checked={confirmedPublic} onChange={(e) => setConfirmedPublic(e.target.checked)} />
                                    </div>
                                )}
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
                                        <p role="note" className="type-small text-muted">{REVIEW_WARNING}</p>
                                        <div><Button onClick={() => setPreviewing(true)} disabled={!result}>Preview what a visitor sees</Button></div>
                                    </>
                                )}
                            </section>

                            {createError && <Notice tone="failed" title={createError} />}
                            <p className="type-small text-muted">{COPY_WARNING}</p>
                        </div>
                    )
                ) : (
                    <div className="flex flex-col gap-16">
                        <Notice tone="info" title={OFFLINE_NOTICE} />
                        <p role="note" className="type-small text-retrying">{OFFLINE_WARNING}</p>
                        <SwitchRow label="Include the agent's activity trail (tool calls and results)" helper={TRAIL_HELP} checked={includeTrail} onCheckedChange={setIncludeTrail} />

                        <section className="flex flex-col gap-6" aria-label="What will be exported">
                            <h3 className="type-ui-medium text-fg">What will be exported</h3>
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
                                    <p role="note" className="type-small text-muted">{REVIEW_WARNING}</p>
                                        <div><Button onClick={() => setPreviewing(true)} disabled={!result}>Preview what a visitor sees</Button></div>
                                </>
                            )}
                        </section>

                        {exportError && <Notice tone="failed" title={exportError} />}
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
