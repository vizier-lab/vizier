import { Button, DreamJournalEntry, KeyValueList, LoadMore, NightStrip, Notice, PanelHeader } from '@vizier/design'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useParams, useSearchParams } from 'react-router'
import {
    dreamCycles,
    dreamNights,
    dreamStatus,
    triggerDream,
    type DreamCycle,
    type DreamNight,
    type DreamStatus,
} from '../api/endpoints/dream'
import { cost } from '../lib/format'
import { absoluteTime } from '../lib/format'
import { changeHref, changeText, cycleSummary, cycleWhen, nightHint, nightLabel } from '../lib/dreams'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const POLL_MS = 3000

export default function AgentDream() {
    const { agentId = '' } = useParams()
    const [params, setParams] = useSearchParams()
    const selected = params.get('cycle')

    const [nights, setNights] = useState<DreamNight[]>([])
    const [unattributed, setUnattributed] = useState(0)
    const [cycles, setCycles] = useState<DreamCycle[]>([])
    const [next, setNext] = useState<string | null>(null)
    const [status, setStatus] = useState<DreamStatus | null>(null)
    const [loading, setLoading] = useState(false)
    const entryRefs = useRef(new Map<string, HTMLElement>())

    const refresh = useCallback(async () => {
        const [n, c, s] = await Promise.allSettled([dreamNights(agentId), dreamCycles(agentId), dreamStatus(agentId)])
        if (n.status === 'fulfilled') (setNights(n.value.nights), setUnattributed(n.value.unattributed))
        if (c.status === 'fulfilled') (setCycles(c.value.cycles), setNext(c.value.next))
        if (s.status === 'fulfilled') setStatus(s.value)
    }, [agentId])

    useEffect(() => {
        void refresh()
    }, [refresh])

    // While a dream runs, follow it.
    const running = status?.status !== undefined && status.status !== 'idle'
    useEffect(() => {
        if (!running) return
        const t = setInterval(() => void refresh(), POLL_MS)
        return () => clearInterval(t)
    }, [running, refresh])

    const dreamNow = async () => {
        try {
            await triggerDream(agentId)
            toast.info('Dream started', 'It runs in the background.')
            void refresh()
        } catch (e) {
            toast.error("Couldn't start a dream", e instanceof Error ? e.message : undefined)
        }
    }

    const more = async () => {
        if (!next) return
        setLoading(true)
        try {
            const page = await dreamCycles(agentId, next)
            setCycles((c) => [...c, ...page.cycles])
            setNext(page.next)
        } finally {
            setLoading(false)
        }
    }

    const select = (cycleId: string | null) => {
        setParams((p) => {
            const q = new URLSearchParams(p)
            if (cycleId) q.set('cycle', cycleId)
            else q.delete('cycle')
            return q
        }, { replace: true })
        if (cycleId) entryRefs.current.get(cycleId)?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    }

    const selectedNight = nights.findIndex((n) => n.cycle_id === selected)
    const last = cycles.find((c) => c.state !== 'running')

    return (
        <PageFrame
            title="Dreams"
            meta={status?.last_dream ? `last dream ${absoluteTime(status.last_dream)}${status.next_dream ? ` · next ${absoluteTime(status.next_dream)}` : ''}` : status?.next_dream ? `next dream ${absoluteTime(status.next_dream)}` : undefined}
            actions={
                <Button variant="primary" icon="dreams" loading={running} onClick={() => void dreamNow()}>
                    {running ? 'Dreaming…' : 'Dream now'}
                </Button>
            }
        >
            {nights.length > 0 && (
                <NightStrip
                    nights={nights.map((n, i) => ({
                        memories: n.memories,
                        core: n.core,
                        live: n.state === 'running',
                        hint: nightHint(n),
                        label: nightLabel(n, i, nights.length),
                    }))}
                    selected={selectedNight >= 0 ? selectedNight : undefined}
                    onSelectNight={(i) => nights[i].cycle_id && select(nights[i].cycle_id)}
                />
            )}
            {unattributed > 0 && (
                <Notice tone="info" title={`${unattributed} older change${unattributed === 1 ? '' : 's'} can't be placed on a night`}>
                    They were written by a dream before cycles were recorded and fall outside every cycle's time.
                </Notice>
            )}

            <div className="grid grid-cols-1 gap-24 xl:grid-cols-[1fr_320px]">
                <section aria-label="Journal" className="flex flex-col gap-16">
                    <PanelHeader title="Journal" level={2} />
                    {cycles.length === 0 && <p className="type-small text-muted">No dreams yet. The first one runs on its schedule, or press Dream now.</p>}
                    {cycles.map((c) => (
                        <div key={c.cycle_id} ref={(el) => void (el ? entryRefs.current.set(c.cycle_id, el) : entryRefs.current.delete(c.cycle_id))} className={selected === c.cycle_id ? 'rounded-xl outline-2 outline-offset-4 outline-focus' : undefined}>
                            <DreamJournalEntry
                                when={cycleWhen(c)}
                                state={c.state}
                                meta={[c.model, c.cost_usd != null ? cost(c.cost_usd) : null].filter(Boolean).join(' · ') || undefined}
                                extraction={`${c.sessions_read} conversation${c.sessions_read === 1 ? '' : 's'} read`}
                                consolidation={cycleSummary(c)}
                                changes={[
                                    ...c.changes.map((ch) => ({ kind: ch.kind, text: changeText(ch), href: changeHref(agentId, ch) })),
                                    ...(c.more ? [{ kind: 'revised' as const, text: `…and ${c.more} more` }] : []),
                                ]}
                            />
                        </div>
                    ))}
                    {next && <LoadMore label="Load older" loading={loading} onLoad={() => void more()} />}
                </section>

                <aside aria-label="Dream status" className="flex flex-col gap-16">
                    <PanelHeader title="Status" level={2} />
                    <KeyValueList
                        items={[
                            { label: 'State', value: status?.status ?? '–' },
                            { label: 'Dream model', value: status?.dream_model ? `${status.dream_provider ?? ''}/${status.dream_model}` : 'the agent’s own' },
                            { label: 'Last cost', value: last?.cost_usd != null ? cost(last.cost_usd) : '–' },
                            { label: 'Reflects on', value: 'conversations since the last dream' },
                        ]}
                    />
                    <p className="type-small text-faint">While it dreams, the agent can read and write memory and CORE, and nothing else.</p>
                </aside>
            </div>
        </PageFrame>
    )
}
