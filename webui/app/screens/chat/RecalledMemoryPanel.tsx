import { EmptyState, PanelHeader, RecalledMemoryRow, TextLink } from '@vizier/design'
import { useMemo, useState } from 'react'
import type { ChatMessage, RecalledPassage } from '../../api/frames'

const SHOWN = 5

export type Recalled = RecalledPassage & { origin: 'auto' | 'search'; uid: string }

/** Every passage recalled in `entries` (auto and search), newest first, one row per passage. */
export function recalledFrom(entries: ChatMessage[]): Recalled[] {
    const out: Recalled[] = []
    for (const e of entries) {
        const r = e.content.Recall
        if (r) for (const p of r.passages) out.push({ ...p, origin: r.origin, uid: e.uid })
    }
    return out.reverse()
}

type Props = {
    agentId: string
    recalled: Recalled[]
}

/** Memory the agent was given or searched for, with links into Memory (FR-018e). */
export function RecalledMemoryPanel({ agentId, recalled }: Props) {
    const [all, setAll] = useState(false)
    const rows = useMemo(() => (all ? recalled : recalled.slice(0, SHOWN)), [all, recalled])
    const href = (p: Recalled) =>
        `/${agentId}/memory/${encodeURIComponent(p.bundle)}?doc=${encodeURIComponent(p.path)}`

    return (
        <section aria-label="Recalled memory" className="flex flex-col gap-8">
            <PanelHeader title="Recalled memory" meta={`${recalled.length} passage${recalled.length === 1 ? '' : 's'}`} level={3} />
            {recalled.length === 0 ? (
                <EmptyState kind="empty" icon="memory" title="Nothing recalled yet" message="Passages the agent reads from memory appear here." />
            ) : (
                <>
                    {rows.map((p, i) => (
                        <RecalledMemoryRow key={`${p.uid}-${p.bundle}-${p.path}-${p.ordinal}-${i}`} bundle={p.bundle} path={`${p.path}#${p.ordinal}`} score={p.score} href={href(p)} />
                    ))}
                    {recalled.length > SHOWN && (
                        <TextLink onClick={() => setAll((a) => !a)}>{all ? 'Show fewer' : `Show all ${recalled.length}`}</TextLink>
                    )}
                </>
            )}
        </section>
    )
}
