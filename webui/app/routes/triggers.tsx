import { Button, EmptyState, FilterTabs, ProgressBar, SearchField, Skeleton, SortSelect, Status, Switch, Table, TableRow } from '@vizier/design'
import { useEffect, useState } from 'react'
import { Navigate, useNavigate, useParams, useSearchParams } from 'react-router'
import { listTriggers, triggerKinds, updateTrigger, type Trigger, type TriggerKind } from '../api/endpoints/triggers'
import { canManageTriggers, canPublishWebhooks } from '../lib/access'
import { useHere } from '../lib/returnTo'
import { counts, FILTER_LABELS, intervalText, kindOf, latest, matches, sorted, SORTS, TRIGGER_FILTERS } from '../lib/triggers'
import { useMobile } from '../lib/useMobile'
import { PageFrame } from '../screens/shared/PageFrame'
import { useAgents } from '../stores/agents'
import { toast } from '../stores/toasts'
import { useTriggers } from '../stores/triggers'

/** An agent's triggers: what each watches, how it last went, and which wait for a person. */
export default function Triggers() {
    const { agentId = '' } = useParams()
    const [params, setParams] = useSearchParams()
    // The address a notification carries: `?trigger=<slug>&run=<id>`.
    const asked = params.get('trigger')
    if (asked) {
        const run = params.get('run')
        return <Navigate replace to={`/${agentId}/triggers/${encodeURIComponent(asked)}${run ? `?run=${encodeURIComponent(run)}` : ''}`} />
    }
    return <TriggerList key={agentId} agentId={agentId} params={params} setParams={setParams} />
}

function TriggerList({ agentId, params, setParams }: { agentId: string; params: URLSearchParams; setParams: ReturnType<typeof useSearchParams>[1] }) {
    const navigate = useNavigate()
    const here = useHere()
    const mobile = useMobile()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const manages = !!agent && canManageTriggers(agent)
    const publishes = !!agent && canPublishWebhooks(agent)
    const version = useTriggers((s) => s.version[agentId] ?? 0)
    const [triggers, setTriggers] = useState<Trigger[] | null>(null)
    const [kinds, setKinds] = useState<TriggerKind[] | null>(null)
    const [error, setError] = useState<Error | null>(null)

    useEffect(() => {
        triggerKinds().then(setKinds, () => setKinds([]))
    }, [])
    useEffect(() => {
        const ctl = new AbortController()
        listTriggers(agentId, ctl.signal).then(
            (t) => {
                setTriggers(t)
                setError(null)
            },
            (e) => e?.name !== 'AbortError' && setError(e)
        )
        return () => ctl.abort()
    }, [agentId, version])

    const filter = params.get('filter') ?? 'all'
    const q = params.get('q') ?? ''
    const sort = params.get('sort') ?? SORTS[0].value
    const set = (key: string, value: string, def: string) =>
        setParams(
            (prev) => {
                const next = new URLSearchParams(prev)
                if (value && value !== def) next.set(key, value)
                else next.delete(key)
                return next
            },
            { replace: true }
        )

    const open = (t: Trigger) => navigate(`/${agentId}/triggers/${encodeURIComponent(t.slug)}`, { state: here })
    const create = (kind?: string) => navigate(`/${agentId}/triggers/new/edit${kind ? `?kind=${kind}` : ''}`, { state: here })

    const toggle = async (t: Trigger, on: boolean) => {
        try {
            await updateTrigger(agentId, t.slug, { is_active: on })
        } catch (e) {
            toast.error("Couldn't change the trigger", e instanceof Error ? e.message : undefined)
        }
        useTriggers.getState().changed(agentId)
    }

    const n = triggers ? counts(triggers) : null
    const shown = triggers ? sorted(triggers.filter((t) => matches(t, filter, q)), sort) : []
    // A phone keeps the trigger, with how it last went under its name, and its switch.
    const keep = <T,>(cells: T[]) => (mobile ? [cells[0], cells[4]] : cells)

    return (
        <PageFrame
            title="Triggers"
            meta={n ? [`${n.all} trigger${n.all === 1 ? '' : 's'}`, `${n.active} active`, n.needs_you ? `${n.needs_you} need you` : ''].filter(Boolean).join(' · ') : undefined}
            actions={
                manages && (
                    <Button variant="primary" icon="plus" onClick={() => create()}>
                        New trigger
                    </Button>
                )
            }
        >
            {error ? (
                <EmptyState kind="error" title="Couldn't load the triggers" message={error.message} />
            ) : !triggers ? (
                <Skeleton lines={4} />
            ) : triggers.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="zap"
                    title="No triggers yet"
                    message={`A trigger starts a run of ${agent?.name ?? agentId} when something happens, and sends you the reply.`}
                    action={
                        manages && (
                            <div className="flex flex-col items-center gap-16">
                                {(kinds ?? []).map((k) => (
                                    <div key={k.id} className="flex max-w-364 flex-col items-center gap-6">
                                        <p className="type-small-para text-muted">{k.listing.description}.</p>
                                        <Button icon="plus" disabled={k.public && !publishes} onClick={() => create(k.id)}>
                                            New {k.listing.name.toLowerCase()}
                                        </Button>
                                        {k.public && !publishes && <p className="type-small text-faint">You may not open public webhooks on this agent.</p>}
                                    </div>
                                ))}
                            </div>
                        )
                    }
                />
            ) : (
                <div className="flex flex-col gap-16">
                    <div className="flex flex-wrap items-center gap-12">
                        <div className="min-w-180 max-w-md flex-1">
                            <SearchField aria-label="Search triggers" placeholder="Search triggers" value={q} onValueChange={(v) => set('q', v, '')} />
                        </div>
                        <FilterTabs
                            aria-label="Filter triggers"
                            value={filter}
                            onChange={(v) => set('filter', v, 'all')}
                            options={TRIGGER_FILTERS.map((f) => ({ value: f as string, label: FILTER_LABELS[f], count: n?.[f] }))}
                        />
                        <span className="flex-1" />
                        <SortSelect label="Sort" value={sort} options={SORTS} onChange={(v) => set('sort', v, SORTS[0].value)} />
                    </div>
                    {shown.length === 0 ? (
                        <EmptyState kind="no-results" title="Nothing matched" message="Try a different search or filter." />
                    ) : (
                        <Table
                            label="Triggers"
                            columns={keep([
                                { label: 'Trigger' },
                                { label: 'Watches', width: 220 },
                                { label: 'Latest', width: 250 },
                                { label: 'Runs this hour', width: 150 },
                                { label: 'Active', width: mobile ? 84 : 96, align: 'end' as const },
                            ])}
                        >
                            {shown.map((t) => {
                                const kind = kindOf(kinds, t.kind)
                                const now = latest(t)
                                const interval = Number(t.settings.interval_secs ?? kind?.listing.fields.find((f) => f.key === 'interval_secs')?.default)
                                const by = 'agent' in t.requester || t.awaiting_approval ? 'created by the agent' : ''
                                const detail = by || (Array.isArray(t.settings.filter) ? String(t.settings.filter[0]) : '') || (t.triage ? 'triage on' : t.slug)
                                // Switching a webhook on opens its address, which needs its own permission.
                                const locked = !!kind?.public && !publishes && !t.is_active
                                return (
                                    <TableRow
                                        key={t.slug}
                                        onSelect={() => open(t)}
                                        endAligned={[mobile ? 1 : 4]}
                                        cells={keep([
                                            <span key="t" className="flex min-w-0 flex-col">
                                                <span className="truncate type-body-strong text-fg">{t.title}</span>
                                                {mobile ? (
                                                    <Status state={now.state} label={now.label} font="sans" />
                                                ) : (
                                                    <span className="truncate type-small text-muted">{t.instruction.split('\n')[0]}</span>
                                                )}
                                            </span>,
                                            <span key="w" className="flex min-w-0 flex-col gap-2">
                                                <span className="truncate type-small text-fg">
                                                    {kind?.listing.name ?? t.kind}
                                                    {t.monitor && interval ? ` · ${intervalText(interval)}` : ''}
                                                </span>
                                                <span className="truncate type-mono-xs text-faint">{detail}</span>
                                            </span>,
                                            <span key="l" className="block min-w-0 truncate" title={now.detail}>
                                                <Status state={now.state} label={now.label} detail={now.detail} font="sans" />
                                            </span>,
                                            t.awaiting_approval ? (
                                                <span key="r" className="type-small text-faint">–</span>
                                            ) : (
                                                <span key="r" className="flex w-100 flex-col gap-6">
                                                    <span className="type-mono-xs text-muted">{t.hourly_limit === null ? `${t.runs_last_hour} · no limit` : `${t.runs_last_hour} of ${t.hourly_limit}`}</span>
                                                    {t.hourly_limit !== null && (
                                                        <ProgressBar compact label={`${t.runs_last_hour} of ${t.hourly_limit} runs this hour`} value={(t.runs_last_hour / t.hourly_limit) * 100} />
                                                    )}
                                                </span>
                                            ),
                                            <span key="a" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                                                {t.awaiting_approval ? (
                                                    <Button size="sm" onClick={() => open(t)}>
                                                        {manages ? 'Review' : 'View'}
                                                    </Button>
                                                ) : manages ? (
                                                    <Switch checked={t.is_active} disabled={locked} aria-label={`${t.title} active`} onCheckedChange={(on) => void toggle(t, on)} />
                                                ) : (
                                                    <span className="type-small text-muted">{t.is_active ? 'On' : 'Off'}</span>
                                                )}
                                            </span>,
                                        ])}
                                    />
                                )
                            })}
                        </Table>
                    )}
                </div>
            )}
        </PageFrame>
    )
}
