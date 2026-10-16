import { useMemo } from 'react'
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell } from 'recharts'
import type { DailyUsage, UsageActivity } from '../interfaces/types'
import { USAGE_ACTIVITIES, activityMeta } from './usageActivities'
import { formatCost, formatTokens, formatCount } from '../utils/format'

interface Props {
  daily: DailyUsage[]
  /** Dates before this render faded: their dreams, compaction and failed rounds weren't recorded. */
  accountingSince: string | null
}

/** SVG presentation attributes don't take `var(--…)`, so theme colours are read once. */
function resolveColor(color: string): string {
  const m = color.match(/^var\((--[^)]+)\)$/)
  if (!m || typeof document === 'undefined') return color
  return getComputedStyle(document.documentElement).getPropertyValue(m[1]).trim() || '#888'
}

function shortDate(iso: string): string {
  const d = new Date(`${iso}T00:00:00Z`)
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', timeZone: 'UTC' })
}

const DailyTooltip = ({ active, payload, label }: any) => {
  if (!active || !payload?.length) return null
  const row = payload[0].payload
  return (
    <div
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: '6px',
        padding: '8px 12px',
        fontSize: '12px',
      }}
    >
      <p style={{ fontWeight: 600, margin: '0 0 4px' }}>{shortDate(label)}</p>
      <p style={{ margin: 0 }}>
        {formatCost(row.cost)} · {formatTokens(row.tokens)} tokens · {formatCount(row.calls)} calls
      </p>
      {payload
        .filter((p: any) => p.value > 0)
        .map((p: any) => (
          <p key={p.dataKey} style={{ margin: '2px 0 0', color: 'var(--text-secondary)' }}>
            {activityMeta(p.dataKey).label}: {formatCost(p.value)}
          </p>
        ))}
      {row.incomplete && (
        <p style={{ margin: '4px 0 0', color: 'var(--text-tertiary)' }}>Before complete accounting</p>
      )}
    </div>
  )
}

export function UsageDailyChart({ daily, accountingSince }: Props) {
  const present = useMemo(() => {
    const seen = new Set<UsageActivity>()
    daily.forEach((d) => Object.keys(d.by_activity).forEach((a) => seen.add(a as UsageActivity)))
    return USAGE_ACTIVITIES.filter((a) => seen.has(a.id))
  }, [daily])

  const since = accountingSince ? accountingSince.slice(0, 10) : null
  const data = useMemo(
    () =>
      daily.map((d) => {
        const row: Record<string, any> = { date: d.date, cost: null, tokens: 0, calls: 0, incomplete: since != null && d.date < since }
        for (const a of present) {
          const cell = d.by_activity[a.id]
          row[a.id] = cell?.cost_usd ?? 0
          if (cell) {
            row.tokens += cell.tokens
            row.calls += cell.calls
            if (cell.cost_usd != null) row.cost = (row.cost ?? 0) + cell.cost_usd
          }
        }
        return row
      }),
    [daily, present, since]
  )
  const colors = useMemo(() => Object.fromEntries(present.map((a) => [a.id, resolveColor(a.color)])), [present])

  return (
    <div>
      <div style={{ width: '100%', height: 240 }} aria-hidden="true">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" vertical={false} />
            <XAxis
              dataKey="date"
              tickFormatter={shortDate}
              tick={{ fontSize: 11, fill: 'var(--text-tertiary)' }}
              minTickGap={24}
            />
            <YAxis
              tickFormatter={(v) => `$${Number(v).toFixed(v < 1 ? 2 : 0)}`}
              tick={{ fontSize: 11, fill: 'var(--text-tertiary)' }}
              width={48}
            />
            <Tooltip content={<DailyTooltip />} cursor={{ fill: 'var(--hover)' }} />
            {present.map((a) => (
              <Bar key={a.id} dataKey={a.id} stackId="cost" fill={colors[a.id]} isAnimationActive={false}>
                {data.map((row) => (
                  <Cell key={row.date} fillOpacity={row.incomplete ? 0.45 : 1} />
                ))}
              </Bar>
            ))}
          </BarChart>
        </ResponsiveContainer>
      </div>
      <table className="sr-only">
        <caption>Daily estimated cost</caption>
        <thead>
          <tr>
            <th scope="col">Date</th>
            <th scope="col">Cost</th>
          </tr>
        </thead>
        <tbody>
          {data.map((row) => (
            <tr key={row.date}>
              <td>{row.date}</td>
              <td>{formatCost(row.cost)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

/** The chart's legend, built from the same palette. */
export function UsageLegend({ activities }: { activities: UsageActivity[] }) {
  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '14px' }}>
      {activities.map((id) => {
        const a = activityMeta(id)
        return (
          <span key={id} style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '12px', color: 'var(--text-secondary)' }}>
            <span style={{ width: 10, height: 10, borderRadius: 2, background: a.color }} />
            {a.label}
          </span>
        )
      })}
    </div>
  )
}
