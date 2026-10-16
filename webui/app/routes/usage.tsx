import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { useParams, useSearchParams } from 'react-router'
import { getAgentUsage } from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'
import type { BreakdownRow, ConversationRow, ModelRow, UsageActivity, UsageReport } from '../interfaces/types'
import { UsageDailyChart, UsageLegend } from '../components/UsageDailyChart'
import UsageConversationDrawer from '../components/UsageConversationDrawer'
import { MISSING_BEFORE_ACCOUNTING, USAGE_ACTIVITIES, activityMeta } from '../components/usageActivities'
import { formatCost, formatCount, formatPercent, formatTokens } from '../utils/format'

const PERIODS = [
  { id: '7', label: '7 days', days: 7 },
  { id: '14', label: '14 days', days: 14 },
  { id: '30', label: '30 days', days: 30 },
  { id: '90', label: '90 days', days: 90 },
  { id: 'custom', label: 'Custom', days: 0 },
]

type Dimension = 'activity' | 'model' | 'place'

const card: CSSProperties = {
  background: 'var(--surface)',
  border: '1px solid var(--border)',
  borderRadius: '12px',
  padding: '20px 22px',
}

const sectionTitle: CSSProperties = { margin: 0, fontSize: '16px', fontWeight: 600 }

const dateInput: CSSProperties = {
  border: '1px solid var(--border)',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  borderRadius: '8px',
  padding: '6px 8px',
  minHeight: '36px',
  font: 'inherit',
}

function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string
  options: { id: T; label: string }[]
  value: T
  onChange: (v: T) => void
}) {
  return (
    <div role="group" aria-label={label} style={{ display: 'flex', flexWrap: 'wrap', background: 'var(--hover)', borderRadius: '10px', padding: '3px', gap: '2px' }}>
      {options.map((o) => {
        const active = o.id === value
        return (
          <button
            key={o.id}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(o.id)}
            style={{
              border: 0,
              borderRadius: '8px',
              padding: '7px 12px',
              minHeight: '36px',
              fontSize: '13px',
              cursor: 'pointer',
              background: active ? 'var(--surface)' : 'transparent',
              color: active ? 'var(--text-primary)' : 'var(--text-secondary)',
              fontWeight: active ? 600 : 400,
              boxShadow: active ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
            }}
          >
            {o.label}
          </button>
        )
      })}
    </div>
  )
}

function Notice({ tone, children }: { tone: 'warning' | 'info'; children: ReactNode }) {
  const warning = tone === 'warning'
  return (
    <div
      role="note"
      style={{
        display: 'flex',
        gap: '10px',
        alignItems: 'flex-start',
        fontSize: '13px',
        borderRadius: '10px',
        padding: warning ? '12px 14px' : 0,
        background: warning ? 'var(--color-warning-bg)' : 'transparent',
        border: warning ? '1px solid var(--color-warning-border)' : 'none',
        color: warning ? 'var(--color-warning-text)' : 'var(--text-secondary)',
      }}
    >
      <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flex: 'none', marginTop: 1 }}>
        <circle cx="12" cy="12" r="10" />
        <path d={warning ? 'M12 8v4' : 'M12 16v-4'} />
        <path d={warning ? 'M12 16h.01' : 'M12 8h.01'} />
      </svg>
      <span>{children}</span>
    </div>
  )
}

function dateLabel(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

/** The swatch of a Channel row: the activity its place belongs to. */
function placeColor(key: string): string {
  if (key === 'dreams') return activityMeta('dreams').color
  if (key === 'background') return activityMeta('background').color
  if (key.startsWith('task:')) return activityMeta('scheduled').color
  if (key === 'system') return activityMeta('other').color
  return activityMeta('chat').color
}

function modelDetail(m: ModelRow): string {
  if (!m.priced) return 'No known price, not in the cost'
  const parts: string[] = []
  if (m.served_models.length) parts.push(`served ${m.served_models.join(', ')}`)
  if (m.cache_hit_rate != null) parts.push(`cache ${formatPercent(m.cache_hit_rate)}`)
  if (m.tokens > 0 && m.failed_tokens > 0) parts.push(`${formatPercent(m.failed_tokens / m.tokens, 1)} failed`)
  return parts.join(' · ')
}

export default function UsagePage() {
  const { agentId } = useParams()
  const { addToast } = useToastStore()
  const [params, setParams] = useSearchParams()

  const period = PERIODS.some((p) => p.id === params.get('period')) ? (params.get('period') as string) : '30'
  const activity = (USAGE_ACTIVITIES.some((a) => a.id === params.get('activity')) ? params.get('activity') : null) as UsageActivity | null
  const requestedBy = (['activity', 'model', 'place'].includes(params.get('by') ?? '') ? params.get('by') : 'activity') as Dimension
  const by: Dimension = activity && requestedBy === 'activity' ? 'place' : requestedBy
  const customStart = params.get('start') ?? ''
  const customEnd = params.get('end') ?? ''

  const setParam = (key: string, value: string | null) =>
    setParams(
      (p) => {
        const next = new URLSearchParams(p)
        if (value == null) next.delete(key)
        else next.set(key, value)
        return next
      },
      { replace: true }
    )

  const { startDate, endDate } = useMemo(() => {
    if (period === 'custom' && customStart && customEnd) {
      return {
        startDate: new Date(`${customStart}T00:00:00`).toISOString(),
        endDate: new Date(`${customEnd}T23:59:59`).toISOString(),
      }
    }
    const days = PERIODS.find((p) => p.id === period)?.days || 30
    const end = new Date()
    const start = new Date(end)
    start.setDate(start.getDate() - days)
    return { startDate: start.toISOString(), endDate: end.toISOString() }
  }, [period, customStart, customEnd])

  const [report, setReport] = useState<UsageReport | null>(null)
  const [loading, setLoading] = useState(true)
  const [openRow, setOpenRow] = useState<ConversationRow | null>(null)
  const opener = useRef<HTMLButtonElement | null>(null)

  useEffect(() => {
    if (!agentId) return
    let cancelled = false
    setLoading(true)
    getAgentUsage(agentId, startDate, endDate, activity)
      .then((res) => !cancelled && setReport(res.data))
      .catch(() => !cancelled && addToast('error', 'Failed to load usage', 'Please try again'))
      .finally(() => !cancelled && setLoading(false))
    return () => {
      cancelled = true
    }
  }, [agentId, startDate, endDate, activity])

  const closeDrawer = () => {
    setOpenRow(null)
    opener.current?.focus()
  }

  const header = (
    <div className="main-header" style={{ flexWrap: 'wrap', gap: '12px' }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <h3 style={{ margin: 0 }}>Usage</h3>
      </div>
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px' }}>
        <Segmented label="Period" options={PERIODS} value={period} onChange={(v) => setParam('period', v)} />
        {period === 'custom' && (
          <div style={{ display: 'flex', gap: '6px', alignItems: 'center', fontSize: '13px' }}>
            <label>
              <span className="sr-only">From</span>
              <input type="date" style={dateInput} value={customStart} onChange={(e) => setParam('start', e.target.value || null)} />
            </label>
            <span aria-hidden="true">–</span>
            <label>
              <span className="sr-only">To</span>
              <input type="date" style={dateInput} value={customEnd} onChange={(e) => setParam('end', e.target.value || null)} />
            </label>
          </div>
        )}
      </div>
    </div>
  )

  if (!report) {
    return (
      <>
        {header}
        <div className="main-body" style={{ color: 'var(--text-tertiary)' }}>{loading ? 'Loading usage…' : 'No usage data.'}</div>
      </>
    )
  }

  const { summary, previous } = report
  const empty = summary.calls === 0

  const filterIds = USAGE_ACTIVITIES.filter(
    (a) => a.id !== 'other' || activity === 'other' || report.by_activity.some((r) => r.key === 'other')
  )

  // Only when the range really holds days counted before complete accounting started.
  const since = report.accounting_since
  const incompleteDays = since != null && report.daily.some((d) => d.date < since.slice(0, 10) && Object.keys(d.by_activity).length > 0)
  const showIncomplete = incompleteDays && (!activity || MISSING_BEFORE_ACCOUNTING.includes(activity))

  const change = (() => {
    if (!previous) return null
    const now = summary.cost_usd ?? null
    const before = previous.cost_usd ?? null
    if (now != null && before != null && before > 0) return (now - before) / before
    if (previous.tokens > 0) return (summary.tokens - previous.tokens) / previous.tokens
    return null
  })()

  const periodDays = Math.max(1, Math.round((new Date(report.range.end).getTime() - new Date(report.range.start).getTime()) / 86_400_000))
  const top = report.by_activity[0]
  const driver = activity
    ? `${formatPercent(summary.share_of_all)} of all spend`
    : top
      ? `biggest driver: ${top.label} (${formatPercent(top.share)})`
      : null

  const dims: { id: Dimension; label: string }[] = activity
    ? [{ id: 'model', label: 'Model' }, { id: 'place', label: 'Channel' }]
    : [{ id: 'activity', label: 'Activity' }, { id: 'model', label: 'Model' }, { id: 'place', label: 'Channel' }]

  const rows: { row: BreakdownRow; color: string; detail: string | null }[] =
    by === 'model'
      ? report.by_model.map((m) => ({ row: m, color: m.priced ? 'var(--accent-dark)' : 'var(--color-text-muted)', detail: modelDetail(m) }))
      : by === 'place'
        ? report.by_place.map((r) => ({ row: r, color: placeColor(r.key), detail: r.detail }))
        : report.by_activity.map((r) => {
            const meta = activityMeta(r.key)
            return { row: r, color: meta.color, detail: `${meta.description} · ${formatTokens(r.calls ? r.tokens / r.calls : 0)} tokens/call` }
          })
  const maxShare = Math.max(0, ...rows.map((r) => r.row.share ?? 0))

  const chartActivities = USAGE_ACTIVITIES.filter((a) => report.daily.some((d) => d.by_activity[a.id])).map((a) => a.id)
  const failedShare = summary.cost_usd && summary.failed.cost_usd != null ? summary.failed.cost_usd / summary.cost_usd : null
  const activityLabel = activity ? activityMeta(activity).label : null

  return (
    <>
      {header}
      <div className="main-body" style={{ maxWidth: '1180px', margin: '0 auto', width: '100%', opacity: loading ? 0.6 : 1, transition: 'opacity 120ms' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
          {/* Activity filter (FR-021) */}
          <div role="group" aria-label="Activity filter" style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '13px', color: 'var(--text-secondary)', marginRight: '4px' }}>Activity</span>
            {[{ id: null as UsageActivity | null, label: 'All', color: 'var(--color-text-muted)' }, ...filterIds].map((a) => {
              const active = a.id === activity
              return (
                <button
                  key={a.id ?? 'all'}
                  type="button"
                  aria-pressed={active}
                  onClick={() => setParam('activity', a.id)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '6px',
                    borderRadius: '999px',
                    padding: '6px 12px',
                    minHeight: '36px',
                    fontSize: '13px',
                    cursor: 'pointer',
                    border: `1px solid ${active ? 'var(--text-primary)' : 'var(--border)'}`,
                    background: active ? 'var(--text-primary)' : 'var(--surface)',
                    color: active ? 'var(--background)' : 'var(--text-primary)',
                    fontWeight: active ? 500 : 400,
                  }}
                >
                  <span aria-hidden="true" style={{ width: 8, height: 8, borderRadius: '50%', background: a.color }} />
                  {a.label}
                </button>
              )
            })}
          </div>

          {showIncomplete && since && (
            <Notice tone="warning">
              {activity ? (
                <>
                  Before <strong>{dateLabel(since)}</strong>, {activityLabel?.toLowerCase()} calls weren't recorded, so the change against the previous period overstates growth.
                </>
              ) : (
                <>
                  Before <strong>{dateLabel(since)}</strong> only delivered replies were recorded. Dreams, compaction and failed turns are missing for those days, so the change against the previous period overstates growth.
                </>
              )}
            </Notice>
          )}

          {empty ? (
            <div style={{ ...card, textAlign: 'center', color: 'var(--text-tertiary)', padding: '48px 22px' }}>
              {activity ? `No ${activityLabel?.toLowerCase()} usage in this period.` : 'No usage in this period.'}
            </div>
          ) : (
            <>
              {/* Headline + tiles */}
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: '12px' }}>
                <div style={{ ...card, gridColumn: 'span 2', minWidth: 0, display: 'flex', flexDirection: 'column', gap: '10px' }} className="usage-headline">
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                    {activityLabel ? `${activityLabel} · estimated cost` : 'Estimated cost'} · {dateLabel(report.range.start)} – {dateLabel(report.range.end)}
                  </div>
                  <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'baseline', gap: '14px' }}>
                    <span style={{ fontSize: '40px', fontWeight: 600, letterSpacing: '-0.02em', fontVariantNumeric: 'tabular-nums' }}>
                      {summary.cost_usd != null ? `$${summary.cost_usd.toFixed(2)}` : '—'}
                    </span>
                    {summary.cost_usd == null && <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>no known price</span>}
                    {change != null && (
                      <span
                        style={{
                          fontSize: '14px',
                          fontWeight: 600,
                          borderRadius: '999px',
                          padding: '3px 10px',
                          background: change >= 0 ? 'var(--color-error-bg)' : 'var(--color-success-bg)',
                          color: change >= 0 ? 'var(--color-error-text)' : 'var(--color-success-text)',
                        }}
                      >
                        {change >= 0 ? '▲' : '▼'} {formatPercent(Math.abs(change))} vs previous {periodDays} days
                      </span>
                    )}
                  </div>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)', fontVariantNumeric: 'tabular-nums' }}>
                    {formatTokens(summary.tokens)} tokens · {formatCount(summary.calls)} model calls
                    {driver && (
                      <>
                        {' · '}
                        <strong style={{ color: 'var(--text-primary)' }}>{driver}</strong>
                      </>
                    )}
                  </div>
                </div>

                <div style={{ ...card, display: 'flex', flexDirection: 'column', gap: '6px' }}>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Cache hit rate</div>
                  <div style={{ fontSize: '28px', fontWeight: 600, fontVariantNumeric: 'tabular-nums' }}>{formatPercent(summary.cache_hit_rate)}</div>
                  <div style={{ height: 6, background: 'var(--hover)', borderRadius: 3, overflow: 'hidden' }}>
                    <div style={{ width: `${(summary.cache_hit_rate ?? 0) * 100}%`, height: '100%', background: 'var(--accent-dark)' }} />
                  </div>
                  <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>of input tokens served from cache</div>
                </div>

                <div style={{ ...card, display: 'flex', flexDirection: 'column', gap: '6px' }}>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Spent on failed turns</div>
                  <div style={{ fontSize: '28px', fontWeight: 600, fontVariantNumeric: 'tabular-nums', color: summary.failed.tokens > 0 ? 'var(--color-error)' : undefined }}>
                    {summary.failed.cost_usd != null ? `$${summary.failed.cost_usd.toFixed(2)}` : `${formatTokens(summary.failed.tokens)} tokens`}
                  </div>
                  <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                    {failedShare != null && `${formatPercent(failedShare)} of spend · `}
                    {summary.failed.turns} {summary.failed.turns === 1 ? 'turn' : 'turns'} errored, timed out or were aborted
                  </div>
                  {summary.failed.turns > 0 && (
                    <a href="#costliest" style={{ fontSize: '12px', color: 'var(--accent-dark)' }}>
                      See which conversations
                    </a>
                  )}
                </div>
              </div>

              {report.unpriced.tokens > 0 && (
                <Notice tone="info">
                  {formatTokens(report.unpriced.tokens)} tokens on{' '}
                  {report.unpriced.models.map((m, i) => (
                    <span key={m}>
                      {i > 0 && ', '}
                      <code style={{ fontSize: '12px', background: 'var(--hover)', padding: '1px 5px', borderRadius: 4 }}>{m}</code>
                    </span>
                  ))}{' '}
                  have no known price and aren't in the cost.
                </Notice>
              )}

              {/* Daily cost (FR-013) */}
              <section style={{ ...card, display: 'flex', flexDirection: 'column', gap: '16px' }} aria-labelledby="usage-daily">
                <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'space-between', alignItems: 'center', gap: '12px' }}>
                  <h2 id="usage-daily" style={sectionTitle}>Daily cost</h2>
                  <UsageLegend activities={chartActivities} />
                </div>
                <UsageDailyChart daily={report.daily} accountingSince={showIncomplete ? since : null} />
                <div style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
                  Hover a day for its tokens and calls.{showIncomplete && ' Faded days predate complete accounting.'}
                </div>
              </section>

              {/* Where it went (FR-014..FR-016) */}
              <section style={{ ...card, display: 'flex', flexDirection: 'column', gap: '14px' }} aria-labelledby="usage-breakdown">
                <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'space-between', alignItems: 'center', gap: '12px' }}>
                  <h2 id="usage-breakdown" style={sectionTitle}>
                    {activityLabel ? `Where ${activityLabel.toLowerCase()} spend went` : 'Where it went'}
                  </h2>
                  <Segmented label="Break down by" options={dims} value={by} onChange={(v) => setParam('by', v)} />
                </div>
                <div style={{ overflowX: 'auto' }}>
                  <table style={{ width: '100%', minWidth: '640px', borderCollapse: 'collapse', fontVariantNumeric: 'tabular-nums' }}>
                    <thead>
                      <tr style={{ fontSize: '12px', color: 'var(--text-tertiary)', textAlign: 'left' }}>
                        <th style={{ padding: '0 4px 8px', fontWeight: 500 }}>{by === 'model' ? 'Model' : by === 'place' ? 'Channel' : 'Activity'}</th>
                        <th style={{ padding: '0 4px 8px', fontWeight: 500, width: '28%' }}>Share of cost</th>
                        <th style={{ padding: '0 4px 8px', fontWeight: 500, textAlign: 'right' }}>Cost</th>
                        <th style={{ padding: '0 4px 8px', fontWeight: 500, textAlign: 'right' }}>Tokens</th>
                        <th style={{ padding: '0 4px 8px', fontWeight: 500, textAlign: 'right' }}>Calls</th>
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map(({ row, color, detail }) => (
                        <tr key={row.key} style={{ borderTop: '1px solid var(--border)' }}>
                          <td style={{ padding: '12px 4px' }}>
                            <div style={{ display: 'flex', alignItems: 'flex-start', gap: '10px' }}>
                              <span aria-hidden="true" style={{ flex: 'none', width: 10, height: 10, marginTop: 5, borderRadius: 2, background: color }} />
                              <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', minWidth: 0 }}>
                                <span style={{ fontWeight: 500 }}>{row.label}</span>
                                {detail && <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>{detail}</span>}
                              </div>
                            </div>
                          </td>
                          <td style={{ padding: '12px 4px' }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                              <div style={{ flex: 1, height: 8, background: 'var(--hover)', borderRadius: 4, overflow: 'hidden' }}>
                                <div style={{ width: `${maxShare > 0 ? ((row.share ?? 0) / maxShare) * 100 : 0}%`, height: '100%', background: color }} />
                              </div>
                              <span style={{ width: 40, textAlign: 'right', fontSize: '13px', color: 'var(--text-secondary)' }}>{formatPercent(row.share)}</span>
                            </div>
                          </td>
                          <td style={{ padding: '12px 4px', textAlign: 'right', fontWeight: 500 }}>{formatCost(row.cost_usd)}</td>
                          <td style={{ padding: '12px 4px', textAlign: 'right', color: 'var(--text-secondary)' }}>{formatTokens(row.tokens)}</td>
                          <td style={{ padding: '12px 4px', textAlign: 'right', color: 'var(--text-secondary)' }}>{formatCount(row.calls)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>

              {/* Costliest conversations (FR-017) */}
              <section id="costliest" style={{ ...card, display: 'flex', flexDirection: 'column', gap: '14px' }} aria-labelledby="usage-costliest">
                <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'space-between', alignItems: 'baseline', gap: '8px' }}>
                  <h2 id="usage-costliest" style={sectionTitle}>Costliest conversations</h2>
                  <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
                    {activityLabel ? `Top conversations by ${activityLabel.toLowerCase()} cost` : 'Top 10 conversations and scheduled runs in the period'}
                  </span>
                </div>
                {report.costliest.length === 0 ? (
                  <div style={{ padding: '24px 4px', color: 'var(--text-tertiary)', textAlign: 'center' }}>No conversations for this activity in the period.</div>
                ) : (
                  <div style={{ overflowX: 'auto' }}>
                    <table style={{ width: '100%', minWidth: '680px', borderCollapse: 'collapse', fontSize: '14px', fontVariantNumeric: 'tabular-nums' }}>
                      <thead>
                        <tr style={{ fontSize: '12px', color: 'var(--text-tertiary)', textAlign: 'left' }}>
                          <th style={{ padding: '0 4px 8px', fontWeight: 500 }}>#</th>
                          <th style={{ padding: '0 4px 8px', fontWeight: 500 }}>Conversation</th>
                          <th style={{ padding: '0 4px 8px', fontWeight: 500 }}>Activity</th>
                          <th style={{ padding: '0 4px 8px', fontWeight: 500 }}>Model</th>
                          <th style={{ padding: '0 4px 8px', fontWeight: 500, textAlign: 'right' }}>Tokens</th>
                          <th style={{ padding: '0 4px 8px', fontWeight: 500, textAlign: 'right' }}>Cost</th>
                          <th style={{ padding: '0 4px 8px' }}>
                            <span className="sr-only">Details</span>
                          </th>
                        </tr>
                      </thead>
                      <tbody>
                        {report.costliest.map((c, i) => {
                          const meta = activityMeta(c.activity)
                          const failed = (c.failed_share ?? 0) >= 0.5
                          return (
                            <tr key={`${c.channel}/${c.topic ?? ''}`} style={{ borderTop: '1px solid var(--border)' }}>
                              <td style={{ padding: '11px 4px', color: 'var(--text-tertiary)' }}>{i + 1}</td>
                              <td style={{ padding: '11px 4px' }}>
                                <div style={{ fontWeight: 500 }}>
                                  {c.label}
                                  {failed && (
                                    <span style={{ marginLeft: 8, fontSize: '11px', fontWeight: 600, borderRadius: 4, padding: '1px 6px', background: 'var(--color-error-bg)', color: 'var(--color-error-text)' }}>
                                      Failed
                                    </span>
                                  )}
                                </div>
                                <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                                  {c.detail}
                                  {c.detail && ' · '}
                                  {dateLabel(c.last_at)}
                                  {!c.exists && ' · deleted'}
                                </div>
                              </td>
                              <td style={{ padding: '11px 4px', fontSize: '13px' }}>
                                <span style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                                  <span aria-hidden="true" style={{ width: 8, height: 8, borderRadius: '50%', background: meta.color }} />
                                  {meta.label}
                                </span>
                              </td>
                              <td style={{ padding: '11px 4px', fontFamily: 'var(--font-mono)', fontSize: '12px', color: 'var(--text-secondary)' }}>{c.model}</td>
                              <td style={{ padding: '11px 4px', textAlign: 'right', color: 'var(--text-secondary)' }}>{formatTokens(c.tokens)}</td>
                              <td style={{ padding: '11px 4px', textAlign: 'right', fontWeight: 500 }}>{formatCost(c.cost_usd)}</td>
                              <td style={{ padding: '11px 4px', textAlign: 'right' }}>
                                <button
                                  type="button"
                                  className="btn btn-secondary"
                                  aria-label={`Details for ${c.label}`}
                                  onClick={(e) => {
                                    opener.current = e.currentTarget
                                    setOpenRow(c)
                                  }}
                                  style={{ minHeight: '36px' }}
                                >
                                  Details
                                </button>
                              </td>
                            </tr>
                          )
                        })}
                      </tbody>
                    </table>
                  </div>
                )}
              </section>
            </>
          )}
        </div>
      </div>

      {agentId && (
        <UsageConversationDrawer
          agentId={agentId}
          row={openRow}
          startDate={startDate}
          endDate={endDate}
          activity={activity}
          onClose={closeDrawer}
        />
      )}
    </>
  )
}
