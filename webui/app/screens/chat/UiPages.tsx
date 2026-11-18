// The pages a turn drew (specs/063-html-generative-ui, mockups/page-frame.html): the last version
// is the reply, in the design `InteractivePage` frame; a version before it in the same turn (a
// draft that failed its check and was fixed) is a collapsed row that opens read-only.

import { Floating, IconButton, InteractivePage as Frame, useDismiss } from '@vizier/design'
import { useRef, useState, type ReactNode } from 'react'
import type { ChatMessage, Turn } from '../../api/frames'
import { CHANNEL } from '../../api/endpoints/chat'
import { transport } from '../../api/transport'
import { findFile } from '../../lib/generativeUi/media'
import type { Problem } from '../../lib/uiPage/host'
import { topicKey } from '../../stores/connection'
import { useTurns } from '../../stores/turns'
import { PausedPage, usePaused } from './client/PausedPage'
import { SandboxedPage } from './client/SandboxedPage'
import { useChatAreaFullscreen } from './client/useChatAreaFullscreen'

export type PagesContext = {
    agentId: string
    topicId: string
    channel?: string
    /** Pages' state saved in another tab, by page id. */
    pageStates: Record<string, { state: unknown; seq: number }>
}

type Entry = { page_id: string; version: number; title: string; verification?: unknown }

type Verification =
    | { state: 'pending' }
    | { state: 'unverified'; reason?: string }
    | { state: 'clean' }
    | { state: 'problems'; problems?: Problem[] }

const verificationOf = (value: unknown): Verification | null =>
    value && typeof value === 'object' && 'state' in value ? (value as Verification) : null

const fileName = (title: string) => `${title.replace(/[^\w.-]+/g, '-').replace(/^-+|-+$/g, '') || 'page'}.html`

/** The turn's pages, or nothing. */
export function renderPages(turn: Turn, ctx: PagesContext): ReactNode {
    const pages = turn.pages.flatMap((entry: ChatMessage) => (entry.content.UiPage ? [entry.content.UiPage as Entry] : []))
    if (pages.length === 0) return null
    const last = pages[pages.length - 1]
    return (
        <div className="flex flex-col gap-8">
            {pages.slice(0, -1).map((page) => (
                <EarlierVersion key={`${page.page_id}:${page.version}`} page={page} ctx={ctx} />
            ))}
            <PageReply key={`${last.page_id}:${last.version}`} page={last} ctx={ctx} />
        </div>
    )
}

/** The largest conversation file a page may load through `vizier.files.url`. */
const MAX_PAGE_FILE_BYTES = 8 * 1024 * 1024

/** A conversation file by name as a `data:` URL: the frame has an opaque origin, so it can use
 * neither the WebUI's own file URLs nor its `blob:` URLs. */
async function fileAsDataUrl(agentId: string, topicId: string, name: string): Promise<string> {
    const files = useTurns.getState().topics[topicKey(agentId, topicId)]?.files ?? []
    const file = findFile(files, name)
    if (!file) throw new Error(`no file called ${name} in this conversation`)
    if (file.size > MAX_PAGE_FILE_BYTES) throw new Error(`${name} is larger than ${MAX_PAGE_FILE_BYTES / 1024 / 1024} MB`)
    const res = await transport.fetchFile(`/files/${encodeURIComponent(file.file_id)}`)
    const blob = await res.blob()
    return new Promise((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result))
        reader.onerror = () => reject(new Error(`${name} could not be read`))
        reader.readAsDataURL(blob.type ? blob : new Blob([blob], { type: file.mime_type }))
    })
}

function PageReply({ page, ctx }: { page: Entry; ctx: PagesContext }) {
    const wrapperRef = useRef<HTMLDivElement>(null)
    const { area, fullscreen, toggle } = useChatAreaFullscreen(wrapperRef)
    const [problems, setProblems] = useState<Problem[]>([])
    const verification = verificationOf(page.verification)
    const channel = ctx.channel ?? CHANNEL
    const [paused, run] = usePaused(page.page_id, page.version)

    const download = () =>
        void transport.saveFile(
            `/agents/${ctx.agentId}/channel/${encodeURIComponent(channel)}/topic/${encodeURIComponent(ctx.topicId)}/pages/${encodeURIComponent(page.page_id)}/versions/${page.version}`,
            fileName(page.title)
        )

    return (
        <div ref={wrapperRef} className={fullscreen ? 'fixed z-40 bg-ground p-16' : undefined} style={area ?? undefined}>
            <Frame
                title={page.title}
                fill={fullscreen}
                onFullscreen={toggle}
                label={
                    <span className="flex items-center gap-6">
                        {paused && (
                            <span
                                className="rounded-pill border border-retrying-line px-6 type-mono-xs text-retrying"
                                title="This version froze the tab last time it ran"
                            >
                                paused
                            </span>
                        )}
                        {!paused && verification?.state === 'unverified' && (
                            <span
                                className="rounded-pill border border-border-strong px-6 type-mono-xs text-faint"
                                title="Not checked for errors: no tab was open when it was made"
                            >
                                unchecked
                            </span>
                        )}
                        <span>v{page.version}</span>
                    </span>
                }
                actions={
                    <>
                        {problems.length > 0 && <ProblemsButton problems={problems} />}
                        <IconButton variant="ghost" size="sm" icon="download" aria-label="Download the page source" onClick={download} />
                    </>
                }
            >
                {paused ? (
                    <PausedPage onRun={run} />
                ) : (
                    <SandboxedPage
                        agentId={ctx.agentId}
                        topicId={ctx.topicId}
                        channel={ctx.channel}
                        pageId={page.page_id}
                        version={page.version}
                        title={page.title}
                        fill={fullscreen}
                        state={ctx.pageStates[page.page_id]?.state}
                        onProblem={(problem) => setProblems((held) => (held.length >= 20 ? held : [...held, problem]))}
                        onSaveState={(state) => useTurns.getState().savePageState(ctx.agentId, ctx.topicId, page.page_id, state)}
                        onSend={(text, data) =>
                            useTurns.getState().clientEvent(ctx.agentId, ctx.topicId, text, data, undefined, { page_id: page.page_id, version: page.version })
                        }
                        resolveFile={(name) => fileAsDataUrl(ctx.agentId, ctx.topicId, name)}
                    />
                )}
            </Frame>
        </div>
    )
}

/** The error indicator and its list (FR-021): Escape, an outside press or × closes it. */
function ProblemsButton({ problems }: { problems: Problem[] }) {
    const [open, setOpen] = useState(false)
    const anchor = useRef<HTMLSpanElement>(null)
    const panel = useRef<HTMLDivElement>(null)
    useDismiss(open, () => setOpen(false), panel, anchor)
    return (
        <span ref={anchor} className="relative">
            <IconButton
                variant="ghost"
                size="sm"
                icon="alert"
                className="text-failed"
                aria-label={`${problems.length} problem${problems.length === 1 ? '' : 's'} on this page`}
                aria-expanded={open}
                onClick={() => setOpen((o) => !o)}
            />
            <span className="pointer-events-none absolute -top-2 right-0 min-w-14 rounded-pill bg-failed px-3 text-center type-mono-xs text-ground">
                {problems.length}
            </span>
            {open && (
                <Floating anchor={anchor} align="end" width={340}>
                    <div ref={panel} role="dialog" aria-label="Problems on this page" className="rounded-lg border border-border-strong bg-panel p-10 shadow-lg">
                        <div className="mb-6 flex items-center gap-6">
                            <span className="flex-1 type-ui-medium text-fg">
                                {problems.length} problem{problems.length === 1 ? '' : 's'} on this page
                            </span>
                            <IconButton variant="ghost" size="sm" icon="close" aria-label="Close" onClick={() => setOpen(false)} />
                        </div>
                        {problems.map((p, i) => (
                            <div key={i} className="border-t border-border py-8">
                                <code className="block whitespace-pre-wrap type-mono-xs text-failed-text">{p.message}</code>
                                <span className="type-mono-xs text-faint">
                                    {p.kind}
                                    {p.line ? ` · line ${p.line}${p.column ? `:${p.column}` : ''}` : ''}
                                    {p.detail ? ` · ${p.detail}` : ''}
                                </span>
                            </div>
                        ))}
                    </div>
                </Floating>
            )}
        </span>
    )
}

/** A version before the turn's last: a dashed row that opens it read-only. */
function EarlierVersion({ page, ctx }: { page: Entry; ctx: PagesContext }) {
    const [open, setOpen] = useState(false)
    const [paused, run] = usePaused(page.page_id, page.version)
    const verification = verificationOf(page.verification)
    const count = verification?.state === 'problems' ? (verification.problems?.length ?? 0) : 0
    return (
        <div className="flex flex-col gap-6">
            <button
                type="button"
                onClick={() => setOpen((o) => !o)}
                aria-expanded={open}
                className="flex items-center gap-8 rounded-md border border-dashed border-failed-line bg-failed-bg px-10 py-6 text-left type-ui text-failed-text"
            >
                <span className="flex-1 text-soft">
                    {page.title} <span className="type-mono-xs text-faint">v{page.version}</span>
                </span>
                <span>{count > 0 ? `${count} problem${count === 1 ? '' : 's'}` : 'replaced'}</span>
            </button>
            {open && (
                <Frame title={`${page.title} (v${page.version})`} label={<span>read-only</span>}>
                    {paused ? (
                        <PausedPage onRun={run} />
                    ) : (
                        <SandboxedPage agentId={ctx.agentId} topicId={ctx.topicId} channel={ctx.channel} pageId={page.page_id} version={page.version} title={page.title} />
                    )}
                </Frame>
            )}
        </div>
    )
}
