// The viewer behind `/share/:id`, the exported file and the dialog's preview: one component, read
// only. It draws a parsed `Snapshot` and nothing else: no composer, no send, no reactions, no
// delete. Every string is a React text child (a title with markup is text).
//
// It reads as the chat does: a person's turns are `UserMessage` bubbles, an agent's turn is its
// `AgentHeader`, one `AgentReply` for what it said, and its work folded into an `ActivityTrail`.
import { ActivityTrail, AgentHeader, AgentReply, Notice, TrailCall, UserMessage } from '@vizier/design'
import { useMemo } from 'react'
import type { Part, Snapshot, Turn } from '../lib/share/schema'
import { mediaFiles, mediaSrc } from './media'
import { SharedPage } from './SharedPage'
import { SharedUi } from './SharedUi'
import { ViewerMarkdown } from './ViewerMarkdown'

export const READ_ONLY_LINE = "This is a copy; the conversation can't continue here."

function dateText(ms: number): string {
    return ms > 0 ? new Date(ms).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' }) : ''
}

function timeText(ms: number): string {
    return ms > 0 ? new Date(ms).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) : ''
}

function Placeholder({ label }: { label: string }) {
    return <p role="note" className="rounded-md border border-dashed border-border-strong px-12 py-8 type-small text-faint">{label || 'Content that cannot be shown'}</p>
}

function PartView({ part, snapshot, files, size = 'reply' }: { part: Part; snapshot: Snapshot; files: ReturnType<typeof mediaFiles>; size?: 'reply' | 'document' }) {
    switch (part.kind) {
        case 'text':
            return <ViewerMarkdown size={size}>{part.markdown}</ViewerMarkdown>
        case 'media': {
            const src = mediaSrc(snapshot.media[part.key])
            return src ? (
                <img src={src} alt={part.alt ?? ''} referrerPolicy="no-referrer" className="max-w-full rounded-md" style={{ objectFit: 'contain' }} />
            ) : (
                <Placeholder label="An image that cannot be shown" />
            )
        }
        case 'ui':
            return <SharedUi part={part} files={files} />
        case 'page':
            return <SharedPage part={part} />
        case 'question':
            return (
                <div className="flex flex-col gap-8 rounded-md border border-border-strong px-12 py-8" aria-label="Question (read only)">
                    {part.title && <p className="type-ui-medium text-fg">{part.title}</p>}
                    {part.description && <p className="type-small text-muted">{part.description}</p>}
                    {part.items.map((item) => (
                        <div key={item.id} className="flex flex-col gap-2">
                            <p className="type-ui text-fg">{item.prompt}</p>
                            <p className="type-small text-muted">
                                {item.answer ? `Answer: ${item.answer.label ?? (Array.isArray(item.answer.value) ? item.answer.value.join(', ') : String(item.answer.value ?? item.answer.state))}` : 'Not answered'}
                            </p>
                        </div>
                    ))}
                </div>
            )
        case 'placeholder':
            return <Placeholder label={part.label} />
        default:
            return <Placeholder label="Content that this version cannot show" />
    }
}

function TrailView({ turn }: { turn: Turn }) {
    if (!turn.trail?.length) return null
    const count = turn.trail.length
    return (
        // Folded like a finished turn's trail in the chat: the head opens the steps.
        <ActivityTrail state={turn.trail.some((e) => e.ok === false) ? 'failed' : 'done'} title={`Activity (${count} ${count === 1 ? 'step' : 'steps'})`} defaultExpanded={false}>
            {turn.trail.map((e, i) => (
                <TrailCall key={i} name={e.name} state={e.ok === false ? 'failed' : 'done'} defaultOpen>
                    <pre className="whitespace-pre-wrap break-words type-mono-xs text-muted">{JSON.stringify(e.arguments)}</pre>
                    {e.result && <pre className="whitespace-pre-wrap break-words type-mono-xs text-soft">{e.result}</pre>}
                </TrailCall>
            ))}
        </ActivityTrail>
    )
}

function PersonTurn({ turn, snapshot, files }: { turn: Turn; snapshot: Snapshot; files: ReturnType<typeof mediaFiles> }) {
    return (
        <section aria-label="Person" className="flex min-w-0 flex-col">
            <UserMessage name="Person" seed="Person" time={timeText(turn.at)}>
                <div className="flex min-w-0 flex-col gap-8">
                    {turn.parts.map((part, i) => (
                        <PartView key={i} part={part} snapshot={snapshot} files={files} size="document" />
                    ))}
                </div>
            </UserMessage>
        </section>
    )
}

function AgentTurn({ turn, name, snapshot, files }: { turn: Turn; name: string; snapshot: Snapshot; files: ReturnType<typeof mediaFiles> }) {
    return (
        <section aria-label={name} className="flex min-w-0 flex-col gap-10">
            <AgentHeader agentId={name} name={name} time={timeText(turn.at)} />
            {turn.parts.length > 0 && (
                <AgentReply>
                    <div className="flex min-w-0 flex-col gap-16">
                        {turn.parts.map((part, i) => (
                            <PartView key={i} part={part} snapshot={snapshot} files={files} />
                        ))}
                    </div>
                </AgentReply>
            )}
            <TrailView turn={turn} />
        </section>
    )
}

export function SnapshotView({ snapshot, banner }: { snapshot: Snapshot; banner?: string }) {
    const files = useMemo(() => mediaFiles(snapshot.media), [snapshot.media])
    const agent = snapshot.source.agent_name || 'Agent'
    const when = dateText(snapshot.created_at)
    return (
        <div className="flex min-h-screen w-full min-w-0 flex-col overflow-x-clip bg-ground text-fg" data-testid="snapshot-view">
            {/* One column for the header, the conversation and the footer: the padding is the page's,
                the width limit the column's, so every edge lines up on every screen (FR-004). */}
            <header className="border-b border-border-strong">
                <div className="px-24 py-16">
                    <div className="mx-auto flex w-full max-w-4xl xl:max-w-5xl 2xl:max-w-6xl flex-col gap-8">
                        <div className="flex items-baseline justify-between gap-16">
                            <h1 className="min-w-0 flex-1 [overflow-wrap:anywhere] type-heading text-fg">{snapshot.title || 'Shared conversation'}</h1>
                            <span className="shrink-0 type-mono-xs text-faint">Shared snapshot</span>
                        </div>
                        <p className="[overflow-wrap:anywhere] type-mono-xs text-faint">
                            Read-only copy{when ? ` · ${when}` : ''} · activity trail {snapshot.include_trail ? 'included' : 'not included'}
                        </p>
                        {/* FR-007: the author's name is here only when they chose to show it. */}
                        {snapshot.source.author && <p className="[overflow-wrap:anywhere] type-small text-muted">Shared by {snapshot.source.author}</p>}
                        {banner && <Notice tone="info" title={banner} />}
                    </div>
                </div>
            </header>
            <main aria-label="Shared conversation" className="flex min-w-0 flex-1 flex-col px-24 py-24">
                <div className="mx-auto flex w-full min-w-0 max-w-4xl xl:max-w-5xl 2xl:max-w-6xl flex-col gap-20">
                    {snapshot.turns.map((turn) =>
                        turn.role === 'person' ? (
                            <PersonTurn key={turn.id} turn={turn} snapshot={snapshot} files={files} />
                        ) : (
                            <AgentTurn key={turn.id} turn={turn} name={agent} snapshot={snapshot} files={files} />
                        )
                    )}
                </div>
            </main>
            <footer className="border-t border-border-strong">
                <div className="px-24 py-16">
                    <div className="mx-auto w-full max-w-4xl xl:max-w-5xl 2xl:max-w-6xl">
                        <p className="[overflow-wrap:anywhere] type-small text-muted">{READ_ONLY_LINE}</p>
                    </div>
                </div>
            </footer>
        </div>
    )
}
