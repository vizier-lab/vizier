import { EmptyState, FilterTabs, Meter, Skeleton, KeyValueList, Notice, PanelHeader, SlideOver, SortSelect, StackedBarChart, StatGroup, Table, TableRow, TextLink, motionDurations } from '@vizier/design'
import { motion } from 'motion/react'
import { useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import {
    conversationUsage,
    usageReport,
    type Activity,
    type ConversationUsage,
    type CostlyConversation,
    type ModelRowDto,
    type UsageKind,
    type UsageReport,
} from '../api/endpoints/usage'
import { absoluteTime, abbreviate, cost, count } from '../lib/format'
import {
    ACTIVITY_LABEL,
    KIND_LABEL,
    SERIES_COLOURS,
    alternativesTitle,
    dailyStacks,
    formatUnits,
    isFree,
    priceNote,
    rangeDates,
    shareOf,
    topSeries,
    unpricedMedia,
    versusPrevious,
    type Range,
} from '../lib/usage'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

type Dimension = 'activity' | 'kind' | 'channel' | 'model'

const KINDS: UsageKind[] = ['text', 'image', 'speech', 'transcription', 'embedding']

const usd = (n?: number | null) => (n == null ? '–' : cost(n))

export default function Usage() {
    const { agentId = '' } = useParams()
    const [range, setRange] = useState<Range>('30')
    const [by, setBy] = useState<Dimension>('activity')
    // Spec 065: one kind of call, or all of them.
    const [kind, setKind] = useState<UsageKind | 'all'>('all')
    const kindFilter = kind === 'all' ? undefined : kind
    // Filtered to one kind, the Kind breakdown has one row: show activities instead.
    const dimension: Dimension = by === 'kind' && kindFilter ? 'activity' : by
    // Null means "whatever the data suggests": cost when the report is priced, tokens otherwise.
    const [picked, setPicked] = useState<'cost' | 'tokens' | null>(null)
    const [report, setReport] = useState<UsageReport | null>(null)
    const [error, setError] = useState<string>()
    const [open, setOpen] = useState<{ row: CostlyConversation; usage: ConversationUsage | null } | null>(null)

    const dates = useMemo(() => rangeDates(range === 'custom' ? '30' : range, new Date()), [range])

    useEffect(() => {
        setReport(null)
        setError(undefined)
        usageReport(agentId, dates.start, dates.end, undefined, kindFilter).then(setReport).catch((e) => setError(e.message))
    }, [agentId, dates, kindFilter])

    const openConversation = async (row: CostlyConversation) => {
        setOpen({ row, usage: null })
        try {
            setOpen({ row, usage: await conversationUsage(agentId, row, dates.start, dates.end, kindFilter) })
        } catch (e) {
            toast.error("Couldn't load that conversation's usage", e instanceof Error ? e.message : undefined)
            setOpen(null)
        }
    }

    const rows = report
        ? dimension === 'activity'
            ? report.by_activity
            : dimension === 'kind'
              ? (report.by_kind ?? [])
              : dimension === 'channel'
                ? report.by_place
                : report.by_model
        : []
    const unpricedMediaText = unpricedMedia(report?.unpriced.units)
    const measure = picked ?? (report?.summary.cost_usd != null ? 'cost' : 'tokens')
    const activities = report?.by_activity ?? []
    // Five chart colours and "Other" takes the last: keep one fewer activity so the fold has
    // a colour of its own instead of repeating the fifth series.
    const keep = activities.length > SERIES_COLOURS.length ? SERIES_COLOURS.length - 1 : SERIES_COLOURS.length
    const { top, other } = topSeries(activities, (r) => (measure === 'cost' ? (r.cost_usd ?? 0) : r.tokens), keep)
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
                <div className="flex flex-wrap items-center gap-12">
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
                        label="Kind"
                        value={kind}
                        onChange={(v) => setKind(v as UsageKind | 'all')}
                        options={[{ value: 'all', label: 'All kinds' }, ...KINDS.map((k) => ({ value: k, label: KIND_LABEL[k] }))]}
                    />
                    <SortSelect
                        label="Break down by"
                        value={dimension}
                        onChange={(v) => setBy(v as Dimension)}
                        options={[
                            { value: 'activity', label: 'Activity' },
                            ...(kindFilter ? [] : [{ value: 'kind', label: 'Kind' }]),
                            { value: 'channel', label: 'Channel' },
                            { value: 'model', label: 'Model' },
                        ]}
                    />
                </div>
            }
        >
            {error && <Notice tone="failed" title="Couldn't load usage">{error}</Notice>}
            {!report && !error && (
                <>
                    <Skeleton lines={2} />
                    <Skeleton lines={6} />
                </>
            )}

            {s && (
                <motion.div
                    // A new range or dimension replaces the whole report: it
                    // fades up rather than snapping into place.
                    key={`${range}-${dimension}-${kind}`}
                    initial={{ opacity: 0.7 }}
                    animate={{
                        opacity: 1,
                        transition: { duration: motionDurations.fast },
                    }}
                >
                <StatGroup
                    stats={[
                        {
                            label: kindFilter ? `${KIND_LABEL[kindFilter]} cost` : 'Estimated cost',
                            value: usd(s.cost_usd),
                            sub:
                                kindFilter && s.kind_share_of_all != null
                                    ? `${Math.round(s.kind_share_of_all * 100)}% of all spend`
                                    : versusPrevious(s.cost_usd, report?.previous?.cost_usd, `previous ${range} days`),
                        },
                        { label: 'Tokens', value: count(s.tokens), sub: `${abbreviate(s.input_tokens)} in · ${abbreviate(s.output_tokens)} out` },
                        { label: 'Calls', value: count(s.calls) },
                        { label: 'Cache hit rate', value: s.cache_hit_rate != null ? `${Math.round(s.cache_hit_rate * 100)}%` : '–' },
                        { label: 'Spent on failed turns', value: usd(s.failed.cost_usd), sub: `${s.failed.turns} turn${s.failed.turns === 1 ? '' : 's'}`, tone: s.failed.turns > 0 ? 'failed' : 'default' },
                    ]}
                />
                </motion.div>
            )}

            {report && report.summary.calls === 0 && (
                <EmptyState
                    icon="usage"
                    title={kindFilter ? `No ${KIND_LABEL[kindFilter].toLowerCase()} calls in the last ${range} days` : `No calls in the last ${range} days`}
                    message="Every call this agent makes to a model is counted here: chats, tasks, dreams, compaction, images, speech, transcription and embeddings."
                />
            )}

            {report && report.summary.calls > 0 && (
                <section aria-label="Over time" className="flex flex-col gap-12">
                    <StackedBarChart
                        title={measure === 'cost' ? 'Estimated cost per day' : 'Tokens per day'}
                        action={
                            <FilterTabs
                                aria-label="Measure"
                                value={measure}
                                onChange={setPicked}
                                options={[
                                    { value: 'cost', label: 'Cost' },
                                    { value: 'tokens', label: 'Tokens' },
                                ]}
                            />
                        }
                        series={series}
                        days={dailyStacks(report.daily, measure, keys)}
                        format={measure === 'cost' ? (v) => `$${v.toFixed(2)}` : abbreviate}
                    />
                </section>
            )}

            <div className="grid grid-cols-1 items-start gap-24 xl:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
            {report && report.summary.calls > 0 && (
                <section aria-label="Where it went" className="flex flex-col gap-12">
                    <PanelHeader title="Where it went" level={2} />
                    <Table
                        label="Where it went"
                        columns={[{ label: dimension === 'model' ? 'Model' : dimension === 'channel' ? 'Channel' : dimension === 'kind' ? 'Kind' : 'Activity' }, { label: 'Calls', width: 80, align: 'end' }, { label: 'Tokens', width: 110, align: 'end' }, { label: 'Cost', width: 90, align: 'end' }, { label: 'Share', width: 150 }]}
                    >
                        {rows.map((r) => {
                            const model = dimension === 'model' ? (r as ModelRowDto) : null
                            const note = model
                                ? priceNote(model.price_source, { borrowedFrom: model.borrowed_from, alternatives: model.price_alternatives, sources: report.sources })
                                : undefined
                            const title = model ? alternativesTitle(model.price_alternatives, report.sources) : undefined
                            // A row's media amount in its own unit, beside its detail (spec 065).
                            const second = [r.detail, model?.kind && model.kind !== 'text' ? KIND_LABEL[model.kind] : null, formatUnits(r.units), note]
                                .filter(Boolean)
                                .join(' · ')
                            return (
                                <TableRow
                                    key={r.key}
                                    endAligned={[1, 2, 3]}
                                    cells={[
                                        <span key="l" className="flex flex-col">
                                            <span className="truncate type-body text-fg">
                                                {dimension === 'activity' ? (ACTIVITY_LABEL[r.key as Activity] ?? r.label) : dimension === 'kind' ? (KIND_LABEL[r.key as UsageKind] ?? r.label) : r.label}
                                            </span>
                                            {second && <span className="truncate type-mono-xs text-faint" title={title}>{second}</span>}
                                        </span>,
                                        count(r.calls),
                                        count(r.tokens),
                                        isFree(model?.price_source) ? 'free' : usd(r.cost_usd),
                                        <Meter key="s" share={shareOf(rows, r, measure)} />,
                                    ]}
                                />
                            )
                        })}
                    </Table>
                    {report.unpriced.models.length > 0 && (
                        <p className="type-small text-faint">
                            No known price for {report.unpriced.models.join(', ')}; their {[report.unpriced.tokens > 0 ? `${count(report.unpriced.tokens)} tokens` : null, unpricedMediaText].filter(Boolean).join(' and ')} are counted but not priced.
                        </p>
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

            <p className="type-small text-faint">
                Costs are estimates: from the provider's own price list where it publishes one, then Vizier's price table, then models.dev; a model priced at another provider says it is borrowed. Models that run on this machine are free, and subscription usage is included.
            </p>

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
                                {open.usage.calls.map((c, i) => {
                                    const media = c.kind && c.kind !== 'text' ? [KIND_LABEL[c.kind], formatUnits(c.units), c.variant].filter(Boolean).join(' ') : null
                                    return (
                                        <TableRow
                                            key={i}
                                            endAligned={[2, 3, 4]}
                                            cells={[absoluteTime(c.at), `${c.model}${media ? ` · ${media}` : ''}${c.failed ? ' · failed' : ''}`, count(c.input_tokens), count(c.output_tokens), usd(c.cost_usd)]}
                                        />
                                    )
                                })}
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
