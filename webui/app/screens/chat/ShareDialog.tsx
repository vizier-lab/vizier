// Share this conversation (mockup 1), Link tab only for now. It reads the saved conversation,
// shows what a visitor would get and what was left out, and stores nothing until "Create link".
// Cancel and Preview never call the share API; the preview draws the real viewer on the
// in-memory snapshot.
import { Button, Checkbox, Dialog, Modal, Notice, Radio, Skeleton, SwitchRow } from '@vizier/design'
import { useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { ApiError } from '../../api/client'
import { transport } from '../../api/transport'
import { buildCreateRequest, createShare, revokeShare, searchPeople, sharePath, type Audience, type CreatedShare, type PersonMatch, type ShareListItem } from '../../api/shares'
import { singleFlight } from '../../lib/share/singleFlight'
import { exportOfflineSnapshot } from '../../lib/share/export'
import { SHARE_LIMITS } from '../../lib/share/limits'
import { buildPreview, formatBytes, type Preview } from '../../lib/share/preview'
import { serializeConversation, ShareError, type SerializeResult } from '../../lib/share/serialize'
import { useSession } from '../../stores/session'
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
/** FR-023: the person may leave every picture and sound out to make the share smaller. */
export const MEDIA_HELP = 'On: no pictures or audio are included, which makes the share smaller; a line says where each one was.'
/** FR-007: the author chooses whether their name travels with the snapshot. */
export const AUTHOR_LABEL = 'Show my name on this share'
export const AUTHOR_HELP =
    'Your name appears under the title as the person who shared it. On by default for specific people and anyone signed in on this instance; off for a public link and an offline file.'

/**
 * FR-007's defaults: the author's name is shown by default to the two instance audiences, and
 * hidden by default on a public link and in an offline file.
 */
export function authorDefault(tab: 'link' | 'offline', audience: Audience | null): boolean {
    if (tab === 'offline') return false
    return audience === 'specific' || audience === 'signed_in'
}

/** The person's own choice wins; until they make one, the audience decides. */
export const authorShown = (choice: boolean | null, tab: 'link' | 'offline', audience: Audience | null): boolean =>
    choice ?? authorDefault(tab, audience)

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`

/**
 * The preview for the tab the person is on (FR-046): a link is held to the POST body less its
 * wrapper, an offline file to 20 MB, since it is built in the browser and never sent.
 */
export const tabPreview = (result: SerializeResult, tab: 'link' | 'offline'): Preview =>
    tab === 'offline' ? buildPreview(result, { limitBytes: SHARE_LIMITS.offlineBytes, label: 'offline' }) : buildPreview(result)

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

/** Images and files that were replaced by a placeholder. A picture the person chose to leave out is not a warning. */
export function warningLines(p: Preview): string[] {
    return p.placeholders
        .filter((x) => x.reason !== 'left_out')
        .map((x) => (x.reason === 'oversized' ? `${x.label} was replaced by a placeholder (too large)` : `${x.label} was replaced by a placeholder`))
}

export const linkFor = (origin: string, created: Pick<CreatedShare, 'share_id'>) => `${origin}${sharePath(created.share_id)}`

/** One row of the dialog's link list: what a live share of this conversation says about itself. */
export type LiveLink = Pick<ShareListItem, 'share_id' | 'audience' | 'people_count' | 'created_at'>

/** The conversation's live links as the header read them, so opening the dialog asks for nothing. */
export type ExistingLinks = {
    links: ShareListItem[]
    loading: boolean
    error: string | null
    /** Re-read after a link is created or revoked. */
    reload: () => void
}

export const NO_LINKS_YET = 'No links yet. Create one below.'
/** The line under a link made for named people, whose names this dialog does not have. */
export const SOME_PEOPLE = 'Named people'

export const toLiveLink = (item: ShareListItem): LiveLink => ({
    share_id: item.share_id,
    audience: item.audience,
    people_count: item.people_count,
    created_at: item.created_at,
})

/** What the list shows: what was just created on top, then what the conversation already had, each once; revoked ones drop out. */
export function linkRows(existing: ShareListItem[], added: LiveLink[], revoked: string[]): LiveLink[] {
    const seen = new Set<string>()
    const rows: LiveLink[] = []
    for (const link of [...added, ...existing.map(toLiveLink)]) {
        if (seen.has(link.share_id) || revoked.includes(link.share_id)) continue
        seen.add(link.share_id)
        rows.push(link)
    }
    return rows
}

/** Who a link lets in, in the same words the audience picker uses. */
export function linkAudience(link: LiveLink): string {
    switch (link.audience) {
        case 'public':
            return 'Anyone with the link'
        case 'signed_in':
            return 'Anyone signed in on this instance'
        case 'specific':
            return link.people_count > 0 ? `Specific people (${plural(link.people_count, 'person', 'people')})` : SOME_PEOPLE
    }
}

/** The day a link was made; the API sends milliseconds. */
export const linkCreated = (link: LiveLink) =>
    new Date(link.created_at).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })

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
export async function createShareFor(
    choice: { agentId: string; topicId: string; title: string; audience: Audience; people?: string[] },
    result: SerializeResult,
    origin: string
): Promise<{ link: string; share: CreatedShare }> {
    // No default audience: a public link is never what happens when nothing was chosen (FR-011).
    if (!choice.audience) throw new ShareError('audience', 'Choose who can open the link.')
    const share = await createShare(buildCreateRequest(choice, result))
    return { link: linkFor(origin, share), share }
}

export async function createLink(
    choice: { agentId: string; topicId: string; title: string; audience: Audience; people?: string[] },
    result: SerializeResult,
    origin: string
): Promise<string> {
    return (await createShareFor(choice, result, origin)).link
}

type LinkListProps = {
    links: LiveLink[]
    loading?: boolean
    error?: string | null
    /** The link being revoked, if any: its row shows the work in progress. */
    revoking?: string | null
    onCopy: (shareId: string) => void
    onRevoke: (shareId: string) => void
}

/**
 * The conversation's live links, newest first: each row says who it lets in and when it was made,
 * and copies itself. Revoking asks once in the row, so nothing disappears on a stray press.
 */
export function LiveLinkList({ links, loading = false, error, revoking, onCopy, onRevoke }: LinkListProps) {
    const [confirming, setConfirming] = useState<string | null>(null)
    if (loading && links.length === 0) return <Skeleton variant="lines" lines={2} />
    return (
        <div className="flex flex-col gap-8">
            {error && <Notice tone="failed" title="The links could not be read">{error}</Notice>}
            {links.length === 0 ? (
                error ? null : <p className="type-small text-muted">{NO_LINKS_YET}</p>
            ) : (
                <ul aria-label="Live links to this conversation" className="divide-y divide-border overflow-hidden rounded-lg border border-border">
                    {links.map((link) => (
                        <li key={link.share_id} className="flex items-center gap-12 px-14 py-10 hover:bg-hover">
                            <div className="min-w-0 flex-1">
                                <p className="type-ui text-fg">{linkAudience(link)}</p>
                                <p className="type-small text-muted">
                                    Created {linkCreated(link)} · <span className="type-mono-xs">{sharePath(link.share_id)}</span>
                                </p>
                            </div>
                            {confirming === link.share_id ? (
                                <>
                                    <Button variant="danger" size="sm" loading={revoking === link.share_id} onClick={() => { setConfirming(null); onRevoke(link.share_id) }}>
                                        Revoke link
                                    </Button>
                                    <Button variant="ghost" size="sm" onClick={() => setConfirming(null)}>
                                        Keep
                                    </Button>
                                </>
                            ) : (
                                <>
                                    <Button variant="ghost" size="sm" aria-label={`Copy link: ${linkAudience(link)}`} onClick={() => onCopy(link.share_id)}>
                                        Copy link
                                    </Button>
                                    <Button variant="ghost" size="sm" onClick={() => setConfirming(link.share_id)}>
                                        Revoke
                                    </Button>
                                </>
                            )}
                        </li>
                    ))}
                </ul>
            )}
        </div>
    )
}

type Props = {
    open: boolean
    onClose: () => void
    agentId: string
    topicId: string
    title: string
    agentName: string
    /** The conversation's live links, read once by the header; absent means none are known. */
    existing?: ExistingLinks
}

export function ShareDialog({ open, onClose, agentId, topicId, title, agentName, existing }: Props) {
    const desktop = transport.authMode === 'desktop'
    const person = useSession((s) => s.user)
    const [tab, setTab] = useState<'link' | 'offline'>(desktop ? 'offline' : 'link')
    const [includeTrail, setIncludeTrail] = useState(false)
    const [skipMedia, setSkipMedia] = useState(false)
    // FR-007: null until the person touches the switch, so it keeps following the audience.
    const [authorChoice, setAuthorChoice] = useState<boolean | null>(null)
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
    // Links made in this opening (the header's read predates them) and links revoked in it.
    const [added, setAdded] = useState<LiveLink[]>([])
    const [revoked, setRevoked] = useState<string[]>([])
    const [revoking, setRevoking] = useState<string | null>(null)
    const run = useRef(0)
    // FR-050: confirming twice (double click, key repeat) must create one share.
    const createOnce = useRef(singleFlight<void>())
    // T078: the picker is one keyboard path: the field, then the results with the arrow keys.
    const pickerId = useId()
    const tabId = useId()
    const searchRef = useRef<HTMLInputElement>(null)
    const matchesRef = useRef<HTMLUListElement>(null)

    // FR-007: the switch's value follows the audience until the person sets it themselves. The
    // session carries the signed-in person's username, which is the display name they share with.
    const author = authorShown(authorChoice, tab, audience)
    const authorName = person?.username ?? ''

    // Start clean each time it opens.
    useEffect(() => {
        if (open) {
            setTab(desktop ? 'offline' : 'link')
            setIncludeTrail(false)
            setSkipMedia(false)
            setAuthorChoice(null)
            setAudience(null)
            setPeople([])
            setPeopleQuery('')
            setPeopleMatches([])
            setPeopleError(null)
            setConfirmedPublic(false)
            setCreated(null)
            setAdded([])
            setRevoked([])
            setRevoking(null)
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
        // The result the person just used goes away with the list: keep the keyboard where it was.
        searchRef.current?.focus()
    }

    const removePerson = (userId: string) => {
        setPeople(people.filter((p) => p.user_id !== userId))
        // A chip removed with the keyboard would leave focus on nothing.
        searchRef.current?.focus()
    }

    /** The result buttons, in the order they are drawn. */
    const matchButtons = () => Array.from(matchesRef.current?.querySelectorAll<HTMLButtonElement>('[data-person-match]') ?? [])

    const focusMatch = (index: number) => {
        const items = matchButtons()
        if (items.length === 0) return
        items[Math.max(0, Math.min(items.length - 1, index))].focus()
    }

    const clearPicker = () => {
        setPeopleQuery('')
        setPeopleMatches([])
        setPeopleError(null)
    }

    /** Down from the field opens the results; Escape clears the picker instead of closing the dialog. */
    const onSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'ArrowDown' && peopleMatches.length > 0) {
            e.preventDefault()
            focusMatch(0)
        } else if (e.key === 'Escape' && (peopleQuery !== '' || peopleMatches.length > 0)) {
            e.preventDefault()
            clearPicker()
        }
    }

    /** Arrow keys walk the results, Enter picks (the button's own behaviour), Escape goes back to the field. */
    const onMatchKeyDown = (e: KeyboardEvent<HTMLButtonElement>, index: number) => {
        if (e.key === 'ArrowDown') {
            e.preventDefault()
            focusMatch(index + 1)
        } else if (e.key === 'ArrowUp') {
            e.preventDefault()
            if (index === 0) searchRef.current?.focus()
            else focusMatch(index - 1)
        } else if (e.key === 'Escape') {
            e.preventDefault()
            clearPicker()
            searchRef.current?.focus()
        }
    }

    // Read the conversation (the trail and media switches change what is in it). Reads only.
    useEffect(() => {
        if (!open) return
        const mine = ++run.current
        setLoading(true)
        setLoadError(null)
        serializeConversation({ agentId, topicId, agentName, title, includeTrail, skipMedia, author: author && authorName ? authorName : undefined })
            .then((r) => mine === run.current && setResult(r))
            .catch((e) => mine === run.current && (setResult(null), setLoadError(e instanceof Error ? e.message : 'The conversation could not be read.')))
            .finally(() => mine === run.current && setLoading(false))
        return () => {
            run.current++
        }
    }, [open, agentId, topicId, agentName, title, includeTrail, skipMedia, author, authorName])

    const preview = useMemo(() => (result ? tabPreview(result, tab) : null), [result, tab])

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
                const out = await createShareFor(
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
                setCreated(out.link)
                setAdded((rows) => [
                    {
                        share_id: out.share.share_id,
                        audience: chosen,
                        people_count: chosen === 'specific' ? people.length : 0,
                        created_at: out.share.created_at,
                    },
                    ...rows,
                ])
                existing?.reload()
                void navigator.clipboard?.writeText(out.link).then(() => toast.success('Link copied'), () => undefined)
            } catch (e) {
                setCreateError(createFailure(e))
            } finally {
                setCreating(false)
            }
        },
        [result, agentId, topicId, title, people, existing]
    )

    const create = useCallback(async () => {
        if (!result || !preview?.canShare || !audience) return
        if (audience === 'specific' && people.length === 0) return
        if (audience === 'public' && !confirmedPublic) return
        await createOnce.current(() => doCreate(audience))
    }, [result, preview, audience, people, confirmedPublic, doCreate])

    const copyText = (url: string) => {
        navigator.clipboard?.writeText(url).then(
            () => toast.success('Link copied'),
            () => toast.error("Couldn't copy the link", 'Select it and copy by hand.')
        )
    }

    const copyLink = (shareId: string) => copyText(`${window.location.origin}${sharePath(shareId)}`)

    const revoke = async (shareId: string) => {
        setRevoking(shareId)
        try {
            await revokeShare(shareId)
            setRevoked((ids) => [...ids, shareId])
            toast.success('Share revoked')
            existing?.reload()
        } catch (e) {
            toast.error('Failed to revoke share', e instanceof Error ? e.message : undefined)
        } finally {
            setRevoking(null)
        }
    }

    const audienceLabel =
        audience === 'public'
            ? 'Anyone with the link'
            : audience === 'signed_in'
            ? 'Anyone signed in on this instance'
            : audience === null
            ? 'Not chosen'
            : `Specific people (${plural(people.length, 'person', 'people')})`

    // What the list shows: what the header read, what was made in this opening, minus what was revoked.
    const rows = linkRows(existing?.links ?? [], added, revoked)

    // T078: a tab list behaves as one control: the arrows move between the tabs, Tab leaves the pair.
    const tabOrder = ['link', 'offline'] as const
    const panelId = `${tabId}-panel`
    const onTabKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
        const at = tabOrder.indexOf(tab)
        const next =
            e.key === 'ArrowRight'
                ? tabOrder[(at + 1) % tabOrder.length]
                : e.key === 'ArrowLeft'
                ? tabOrder[(at - 1 + tabOrder.length) % tabOrder.length]
                : e.key === 'Home'
                ? tabOrder[0]
                : e.key === 'End'
                ? tabOrder[tabOrder.length - 1]
                : null
        if (!next) return
        e.preventDefault()
        setTab(next)
        document.getElementById(`${tabId}-${next}`)?.focus()
    }

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
                        id={`${tabId}-link`}
                        aria-selected={tab === 'link'}
                        aria-controls={panelId}
                        tabIndex={tab === 'link' ? 0 : -1}
                        onKeyDown={onTabKeyDown}
                        onClick={() => setTab('link')}
                        className={tab === 'link' ? 'type-ui-medium text-fg underline underline-offset-8 font-semibold' : 'type-ui-medium text-muted hover:text-fg'}
                    >
                        Link
                    </button>
                    <button
                        type="button"
                        role="tab"
                        id={`${tabId}-offline`}
                        aria-selected={tab === 'offline'}
                        aria-controls={panelId}
                        tabIndex={tab === 'offline' ? 0 : -1}
                        onKeyDown={onTabKeyDown}
                        onClick={() => setTab('offline')}
                        className={tab === 'offline' ? 'type-ui-medium text-fg underline underline-offset-8 font-semibold' : 'type-ui-medium text-muted hover:text-fg'}
                    >
                        Offline file
                    </button>
                </div>

                <div role="tabpanel" id={panelId} aria-labelledby={`${tabId}-${tab}`} className="flex flex-col gap-16">
                    {tab === 'link' && !desktop && (
                        <section className="flex flex-col gap-6" aria-label="Links to this conversation">
                            <h3 className="type-ui-medium text-fg">Links to this conversation</h3>
                            <LiveLinkList
                                links={rows}
                                loading={existing?.loading}
                                error={existing?.error}
                                revoking={revoking}
                                onCopy={copyLink}
                                onRevoke={(shareId) => void revoke(shareId)}
                            />
                        </section>
                    )}

                    {tab === 'link' ? (
                        desktop ? (
                            <Notice tone="info" title={DESKTOP_NOTICE} />
                        ) : created ? (
                            <div className="flex flex-col gap-12">
                                <p className="type-ui text-muted">The link is ready and copied.</p>
                                <div className="flex items-center gap-8">
                                    <input readOnly aria-label="Link" value={created} onFocus={(e) => e.currentTarget.select()} className="min-w-0 flex-1 rounded-md border border-border-strong bg-raised px-10 py-6 type-mono-xs text-fg" />
                                    <Button onClick={() => copyText(created)}>Copy</Button>
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
                                                    ref={searchRef}
                                                    type="search"
                                                    aria-label="Add people by username"
                                                    aria-controls={peopleMatches.length > 0 ? `${pickerId}-matches` : undefined}
                                                    placeholder="Search username (at least 2 characters)…"
                                                    value={peopleQuery}
                                                    onChange={(e) => setPeopleQuery(e.target.value)}
                                                    onKeyDown={onSearchKeyDown}
                                                    autoComplete="off"
                                                    spellCheck={false}
                                                    className="w-full rounded-md border border-border-strong bg-raised px-10 py-6 type-ui-xs text-fg placeholder:text-muted focus:border-ink"
                                                />
                                                {/*
                                                    One live region says what the count is and how to walk it, so the
                                                    keyboard path is announced and not only drawn (T078).
                                                */}
                                                <span role="status" aria-live="polite" className="sr-only">
                                                    {searchingPeople
                                                        ? 'Searching…'
                                                        : peopleQuery.trim().length >= 2
                                                        ? peopleMatches.length === 0
                                                            ? 'No matches'
                                                            : `${plural(peopleMatches.length, 'match', 'matches')}. Use the down arrow to choose one.`
                                                        : people.length > 0
                                                        ? `${plural(people.length, 'person', 'people')} chosen`
                                                        : ''}
                                                </span>
                                                {searchingPeople && <span aria-hidden="true" className="absolute right-8 top-8 type-ui-xs text-muted">Searching…</span>}
                                                {peopleMatches.length > 0 && (
                                                    <ul
                                                        ref={matchesRef}
                                                        id={`${pickerId}-matches`}
                                                        aria-label="Matching users"
                                                        className="absolute z-10 mt-4 max-h-48 w-full overflow-auto rounded-md border border-border-strong bg-raised py-4 shadow-lg"
                                                    >
                                                        {peopleMatches.map((match, index) => (
                                                            <li key={match.user_id}>
                                                                <button
                                                                    type="button"
                                                                    data-person-match={match.user_id}
                                                                    onClick={() => addPerson(match)}
                                                                    onKeyDown={(e) => onMatchKeyDown(e, index)}
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
                                <SwitchRow label="Leave out pictures and audio" helper={MEDIA_HELP} checked={skipMedia} onCheckedChange={setSkipMedia} />
                                <SwitchRow label={AUTHOR_LABEL} helper={AUTHOR_HELP} checked={author} onCheckedChange={setAuthorChoice} />

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
                            <SwitchRow label="Leave out pictures and audio" helper={MEDIA_HELP} checked={skipMedia} onCheckedChange={setSkipMedia} />
                            <SwitchRow label={AUTHOR_LABEL} helper={AUTHOR_HELP} checked={author} onCheckedChange={setAuthorChoice} />

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
                </div>
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
