import { BarList, FilterTabs, KeyValueList, Notice, PanelHeader, SlideOver, SortSelect, StackedBarChart, StatGroup, Table, TableRow, TextLink } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import {
    conversationUsage,
    usageReport,
    type Activity,
    type ConversationUsage,
    type CostlyConversation,
    type UsageReport,
} from '../api/endpoints/usage'
import { absoluteTime, abbreviate, cost, count } from '../lib/format'
import { ACTIVITY_LABEL, SERIES_COLOURS, dailyStacks, rangeDates, shareOf, topSeries, versusPrevious, type Range } from '../lib/usage'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

type Dimension = 'activity' | 'channel' | 'model'

const usd = (n?: number | null) => (n == null ? '–' : cost(n))

export default function Usage() {
    const { agentId = '' } = useParams()
    const [range, setRange] = useState<Range>('30')
    const [by, setBy] = useState<Dimension>('activity')
    const [report, setReport] = useState<UsageReport | null>(null)
    const [error, setError] = useState<string>()
    const [open, setOpen] = useState<{ row: CostlyConversation; usage: ConversationUsage | null } | null>(null)

    const dates = useMemo(() => rangeDates(range === 'custom' ? '30' : range, new Date()), [range])

    useEffect(() => {
        setReport(null)
        setError(undefined)
        usageReport(agentId, dates.start, dates.end).then(setReport).catch((e) => setError(e.message))
    }, [agentId, dates])

    const openConversation = async (row: CostlyConversation) => {
        setOpen({ row, usage: null })
        try {
            setOpen({ row, usage: await conversationUsage(agentId, row, dates.start, dates.end) })
        } catch (e) {
            toast.error("Couldn't load that conversation's usage", e instanceof Error ? e.message : undefined)
            setOpen(null)
        }
    }

    const rows = report ? (by === 'activity' ? report.by_activity : by === 'channel' ? report.by_place : report.by_model) : []
    const measure = report?.summary.cost_usd != null ? 'cost' : 'tokens'
    const { top, other } = topSeries(report?.by_activity ?? [], (r) => (measure === 'cost' ? (r.cost_usd ?? 0) : r.tokens))
    const keys = top.map((t) => t.key as Activity)
    const series = [
        ...top.map((t, i) => ({ key: t.key, label: ACTIVITY_LABEL[t.key as Activity] ?? t.label, colour: SERIES_COLOURS[i] })),
        ...(other ? [{ key: 'other', label: 'Other', colour: 'chart-5' as const }] : []),
    ]
    const s = report?.summary

    return (
        <PageFrame
            title="Usage"
            meta={report ? `${absoluteTime(report.range.start, true)} – ${absoluteTime(report.range.end, true)} · ${count(report.summary.calls)} calls` : undefined}
            actions={
                <div className="flex items-center gap-12">
                    <FilterTabs
                        aria-label="Range"
                        value={range}
                        onChange={setRange}
                        options={[
                            { value: '7', label: '7 days' },
                            { value: '30', label: '30 days' },
                        ]}
                    />
                    <SortSelect
                        label="Break down by"
                        value={by}
                        onChange={(v) => setBy(v as Dimension)}
                        options={[
                            { value: 'activity', label: 'Activity' },
                            { value: 'channel', label: 'Channel' },
                            { value: 'model', label: 'Model' },
                        ]}
                    />
                </div>
            }
        >
            {error && <Notice tone="failed" title="Couldn't load usage">{error}</Notice>}
            {report?.accounting_since && Date.parse(report.accounting_since) > Date.parse(report.range.start) && (
                <Notice tone="info" title="Part of this range predates full accounting">
                    Before {absoluteTime(report.accounting_since, true)}, dreams, compaction and failed rounds weren't recorded.
                </Notice>
            )}

            {s && (
                <StatGroup
                    stats={[
                        { label: 'Estimated cost', value: usd(s.cost_usd), sub: versusPrevious(s.cost_usd, report?.previous?.cost_usd, `previous ${range} days`) },
                        { label: 'Tokens', value: count(s.tokens), sub: `${abbreviate(s.input_tokens)} in · ${abbreviate(s.output_tokens)} out` },
                        { label: 'Calls', value: count(s.calls) },
                        { label: 'Cache hit rate', value: s.cache_hit_rate != null ? `${Math.round(s.cache_hit_rate * 100)}%` : '–' },
                        { label: 'Spent on failed turns', value: usd(s.failed.cost_usd), sub: `${s.failed.turns} turn${s.failed.turns === 1 ? '' : 's'}`, tone: s.failed.turns > 0 ? 'failed' : 'default' },
                    ]}
                />
            )}

            {report && (
                <section aria-label="Over time" className="flex flex-col gap-12">
                    <StackedBarChart
                        title={measure === 'cost' ? 'Estimated cost per day' : 'Tokens per day'}
                        series={series}
                        days={dailyStacks(report.daily, measure, keys)}
                        format={measure === 'cost' ? (v) => `$${v.toFixed(2)}` : abbreviate}
                    />
                </section>
            )}

            <div className="grid grid-cols-1 items-start gap-24 xl:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
            {report && (
                <section aria-label="Where it went" className="flex flex-col gap-12">
                    <PanelHeader title="Where it went" level={2} />
                    <Table
                        label="Where it went"
                        columns={[{ label: by === 'model' ? 'Model' : by === 'channel' ? 'Channel' : 'Activity' }, { label: 'Calls', width: 90, align: 'end' }, { label: 'Tokens', width: 110, align: 'end' }, { label: 'Cost', width: 100, align: 'end' }, { label: 'Share', width: 120 }]}
                    >
                        {rows.map((r) => (
                            <TableRow
                                key={r.key}
                                endAligned={[1, 2, 3]}
                                cells={[
                                    <span key="l" className="flex flex-col">
                                        <span className="truncate type-body text-fg">{by === 'activity' ? (ACTIVITY_LABEL[r.key as Activity] ?? r.label) : r.label}</span>
                                        {r.detail && <span className="truncate type-mono-xs text-faint">{r.detail}</span>}
                                    </span>,
                                    count(r.calls),
                                    count(r.tokens),
                                    usd(r.cost_usd),
                                    <BarList key="s" items={[{ label: '', share: shareOf(rows, r, measure), value: '' }]} />,
                                ]}
                            />
                        ))}
                    </Table>
                    {report.unpriced.models.length > 0 && (
                        <p className="type-small text-faint">No catalogue price for {report.unpriced.models.join(', ')}; their {count(report.unpriced.tokens)} tokens are counted but not priced.</p>
                    )}
                </section>
            )}

            {report && report.costliest.length > 0 && (
                <section aria-label="Costliest conversations" className="flex flex-col gap-12">
                    <PanelHeader title="Costliest conversations" level={2} />
                    <Table label="Costliest conversations" columns={[{ label: 'Conversation' }, { label: 'Tokens', width: 90, align: 'end' }, { label: 'Cost', width: 90, align: 'end' }]}>
                        {report.costliest.map((c) => (
                            <TableRow
                                key={`${c.channel}/${c.topic ?? ''}`}
                                endAligned={[1, 2]}
                                onSelect={() => void openConversation(c)}
                                cells={[
                                    <span key="l" className="flex flex-col">
                                        <span className="truncate type-body text-fg">{c.label}</span>
                                        <span className="truncate type-mono-xs text-faint">{ACTIVITY_LABEL[c.activity]} · {absoluteTime(c.last_at)} · {c.model}</span>
                                    </span>,
                                    count(c.tokens),
                                    usd(c.cost_usd),
                                ]}
                            />
                        ))}
                    </Table>
                </section>
            )}
            </div>

            <p className="type-small text-faint">Costs are estimates from the model catalogue's prices. Speech, image generation and embedding calls aren't counted.</p>

            <SlideOver open={open !== null} title={open?.row.label ?? ''} onClose={() => setOpen(null)}>
                <div className="flex flex-col gap-24 p-24">
                    {!open?.usage ? (
                        <p className="type-small text-muted">Loading…</p>
                    ) : (
                        <>
                            {open.usage.insight && <Notice tone="info" title="Worth a look">{open.usage.insight}</Notice>}
                            <KeyValueList
                                items={[
                                    { label: 'Estimated cost', value: usd(open.usage.summary.cost_usd) },
                                    { label: 'Tokens', value: count(open.usage.summary.tokens) },
                                    { label: 'Calls', value: count(open.usage.summary.calls) },
                                    { label: 'Failed turns', value: open.usage.summary.failed.turns },
                                ]}
                            />
                            <Table label="Calls" dense columns={[{ label: 'When' }, { label: 'Model' }, { label: 'In', align: 'end' }, { label: 'Out', align: 'end' }, { label: 'Cost', align: 'end' }]}>
                                {open.usage.calls.map((c, i) => (
                                    <TableRow key={i} endAligned={[2, 3, 4]} cells={[absoluteTime(c.at), `${c.model}${c.failed ? ' · failed' : ''}`, count(c.input_tokens), count(c.output_tokens), usd(c.cost_usd)]} />
                                ))}
                            </Table>
                            {open.usage.more > 0 && <p className="type-small text-faint">…and {open.usage.more} more calls.</p>}
                            {open.usage.link && <TextLink href={open.usage.link.path} arrow="forward">Open the conversation</TextLink>}
                        </>
                    )}
                </div>
            </SlideOver>
        </PageFrame>
    )
}
