// The viewer behind `/share/:id`, the exported file and the dialog's preview: one component, read
// only. It draws a parsed `Snapshot` and nothing else: no composer, no send, no reactions, no
// delete. Every string is a React text child (a title with markup is text).
import { Notice } from '@vizier/design'
import { useMemo } from 'react'
import type { Part, Snapshot, Turn } from '../lib/share/schema'
import { mediaFiles, mediaSrc } from './media'
import { SharedUi } from './SharedUi'
import { ViewerMarkdown } from './ViewerMarkdown'

export const READ_ONLY_LINE = "This is a copy; the conversation can't continue here."

function dateText(ms: number): string {
    return ms > 0 ? new Date(ms).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' }) : ''
}

function Placeholder({ label }: { label: string }) {
    return <p role="note" className="rounded-md border border-dashed border-border-strong px-12 py-8 type-small text-faint">{label || 'Content that cannot be shown'}</p>
}

function PartView({ part, snapshot, files }: { part: Part; snapshot: Snapshot; files: ReturnType<typeof mediaFiles> }) {
    switch (part.kind) {
        case 'text':
            return <ViewerMarkdown>{part.markdown}</ViewerMarkdown>
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
    return (
        <details className="rounded-md border border-border-strong px-12 py-8">
            <summary className="cursor-pointer type-small text-muted">Activity ({turn.trail.length} {turn.trail.length === 1 ? 'step' : 'steps'})</summary>
            <ul className="mt-8 flex flex-col gap-8">
                {turn.trail.map((e, i) => (
                    <li key={i} className="type-mono-xs text-muted">
                        <span className="text-fg">{e.name}</span>{e.ok === false ? ' (failed)' : ''}
                        <pre className="whitespace-pre-wrap break-words">{JSON.stringify(e.arguments)}</pre>
                        {e.result && <pre className="whitespace-pre-wrap break-words">{e.result}</pre>}
                    </li>
                ))}
            </ul>
        </details>
    )
}

export function SnapshotView({ snapshot, banner }: { snapshot: Snapshot; banner?: string }) {
    const files = useMemo(() => mediaFiles(snapshot.media), [snapshot.media])
    const agent = snapshot.source.agent_name || 'Agent'
    const when = dateText(snapshot.created_at)
    return (
        <div className="mx-auto flex min-h-screen w-full max-w-3xl flex-col bg-ground text-fg" data-testid="snapshot-view">
            <header className="flex flex-col gap-4 border-b border-border-strong px-24 py-20">
                <div className="flex items-baseline justify-between gap-16">
                    <h1 className="type-title text-fg">{snapshot.title || 'Shared conversation'}</h1>
                    <span className="shrink-0 type-mono-xs text-faint">Shared snapshot</span>
                </div>
                <p className="type-small text-muted">
                    Read-only copy{when ? ` · ${when}` : ''} · activity trail {snapshot.include_trail ? 'included' : 'not included'}
                </p>
                {banner && <Notice tone="info" title={banner} />}
            </header>
            <main className="flex min-w-0 flex-1 flex-col gap-20 px-24 py-20">
                {snapshot.turns.map((turn) => (
                    <section key={turn.id} className="flex min-w-0 flex-col gap-8" aria-label={turn.role === 'person' ? 'Person' : agent}>
                        <p className="type-ui-medium text-muted">{turn.role === 'person' ? 'Person' : agent}</p>
                        {turn.parts.map((part, i) => (
                            <PartView key={i} part={part} snapshot={snapshot} files={files} />
                        ))}
                        <TrailView turn={turn} />
                    </section>
                ))}
            </main>
            <footer className="border-t border-border-strong px-24 py-16">
                <p className="type-small text-muted">{READ_ONLY_LINE}</p>
            </footer>
        </div>
    )
}
