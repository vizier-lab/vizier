import { EmptyState, PanelHeader, RecalledMemoryRow, TextLink } from '@vizier/design'
import { useMemo, useState } from 'react'
import { useLocation } from 'react-router'
import { docFromChat } from '../../lib/memoryTree'
import type { Recalled } from '../../lib/recall'

const SHOWN = 5

type Props = {
    agentId: string
    recalled: Recalled[]
}

/** Memory the agent was given or searched for, with links into Memory (FR-018e). */
export function RecalledMemoryPanel({ agentId, recalled }: Props) {
    const [all, setAll] = useState(false)
    const rows = useMemo(() => (all ? recalled : recalled.slice(0, SHOWN)), [all, recalled])
    const { pathname } = useLocation()
    const href = (p: Recalled) => docFromChat(agentId, p.bundle, p.path, pathname)

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
