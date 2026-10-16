import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import SlideOver from './SlideOver'
import { getConversationUsage } from '../services/vizier'
import type { ConversationRow, ConversationUsage, UsageActivity, UsageLinkKind } from '../interfaces/types'
import { activityMeta } from './usageActivities'
import { formatCost, formatPercent, formatTokens, formatCount } from '../utils/format'

const LINK_LABELS: Record<UsageLinkKind, string> = {
  chat: 'Open in chat',
  task_run: 'Open run in Tasks',
  dream: 'Open in Dream journal',
  topic: 'Open in launching topic',
}

interface Props {
  agentId: string
  /** The row whose details are open, or null when closed. */
  row: ConversationRow | null
  startDate: string
  endDate: string
  activity: UsageActivity | null
  onClose: () => void
}

function noLinkSentence(reason: ConversationUsage['no_link_reason'], detail: string | null): string {
  if (reason === 'deleted') return 'This conversation was deleted. Its usage still counts.'
  const platform = detail === 'Telegram' ? 'Telegram' : detail === 'Discord' ? 'Discord' : 'These'
  return `${platform} conversations have no page in the WebUI; this breakdown is the only view.`
}

function Figure({ label, value, danger }: { label: string; value: string; danger?: boolean }) {
  return (
    <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '10px 12px', display: 'flex', flexDirection: 'column', gap: '2px' }}>
      <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{label}</span>
      <span style={{ fontSize: '17px', fontWeight: 600, fontVariantNumeric: 'tabular-nums', color: danger ? 'var(--color-error, #b42318)' : undefined }}>
        {value}
      </span>
    </div>
  )
}

/** One conversation's usage, call by call (spec 018 FR-022, FR-023). */
export default function UsageConversationDrawer({ agentId, row, startDate, endDate, activity, onClose }: Props) {
  const [usage, setUsage] = useState<ConversationUsage | null>(null)
  const [error, setError] = useState(false)

  useEffect(() => {
    if (!row) return
    let cancelled = false
    setUsage(null)
    setError(false)
    getConversationUsage(agentId, { channel: row.channel, topic: row.topic, startDate, endDate, activity, limit: 20 })
      .then((res) => !cancelled && setUsage(res.data))
      .catch(() => !cancelled && setError(true))
    return () => {
      cancelled = true
    }
  }, [agentId, row, startDate, endDate, activity])

  const meta = row ? activityMeta(row.activity) : null

  return (
    <SlideOver open={row != null} onClose={onClose} title={row?.label ?? 'Conversation'} width="min(560px, 100vw)">
      {row && meta && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            <span style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '12px', color: 'var(--text-secondary)' }}>
              <span style={{ width: 8, height: 8, borderRadius: '50%', background: meta.color }} />
              {meta.label}
            </span>
            {row.detail && <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>{row.detail}</span>}
          </div>

          {error && <div style={{ color: 'var(--text-secondary)' }}>Couldn't load this conversation's usage.</div>}
          {!usage && !error && <div style={{ color: 'var(--text-tertiary)' }}>Loading…</div>}

          {usage && (
            <>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(110px, 1fr))', gap: '8px' }}>
                <Figure label="Cost" value={formatCost(usage.summary.cost_usd)} />
                <Figure label="Calls" value={formatCount(usage.summary.calls)} />
                <Figure label="Cached input" value={formatPercent(usage.summary.cache_hit_rate)} />
                <Figure
                  label="On failed turns"
                  value={usage.summary.failed.cost_usd != null ? formatCost(usage.summary.failed.cost_usd) : formatTokens(usage.summary.failed.tokens)}
                  danger={usage.summary.failed.tokens > 0}
                />
              </div>

              {usage.insight && (
                <div
                  role="note"
                  style={{
                    background: 'var(--color-error-bg, #fdecea)',
                    border: '1px solid var(--color-error-border, #f2c4bf)',
                    color: 'var(--color-error-text, #6b1810)',
                    borderRadius: '10px',
                    padding: '12px 14px',
                    fontSize: '13px',
                  }}
                >
                  {usage.insight}
                </div>
              )}

              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <h3 style={{ margin: 0, fontSize: '14px', fontWeight: 600 }}>Model calls, most expensive first</h3>
                <div style={{ overflowX: 'auto' }}>
                  <table style={{ width: '100%', tableLayout: 'fixed', borderCollapse: 'collapse', fontSize: '13px', fontVariantNumeric: 'tabular-nums' }}>
                    <thead>
                      <tr style={{ fontSize: '11px', color: 'var(--text-tertiary)', textAlign: 'left' }}>
                        <th style={{ padding: '0 6px 6px 0', fontWeight: 500, width: '92px' }}>Time</th>
                        <th style={{ padding: '0 6px 6px', fontWeight: 500 }}>Call</th>
                        <th style={{ padding: '0 6px 6px', fontWeight: 500, textAlign: 'right', width: '58px' }}>Input</th>
                        <th style={{ padding: '0 6px 6px', fontWeight: 500, textAlign: 'right', width: '58px' }}>Output</th>
                        <th style={{ padding: '0 0 6px 6px', fontWeight: 500, textAlign: 'right', width: '84px' }}>Cost</th>
                      </tr>
                    </thead>
                    <tbody>
                      {usage.calls.map((c, i) => (
                        <tr key={i} style={{ borderTop: '1px solid var(--border)' }}>
                          <td style={{ padding: '7px 6px 7px 0', color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>
                            {new Date(c.at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                          </td>
                          <td style={{ padding: '7px 6px' }}>
                            <div>
                              {activityMeta(c.activity).label}
                              {c.round != null && ` · round ${c.round}`}
                            </div>
                            <div
                              title={c.served_model ?? c.model}
                              style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontSize: '11px', color: c.failed ? 'var(--color-error, #b42318)' : 'var(--text-tertiary)' }}
                            >
                              {c.failed ? 'part of failed turn' : c.served_model ?? c.model}
                            </div>
                          </td>
                          <td style={{ padding: '7px 6px', textAlign: 'right', color: 'var(--text-secondary)' }}>{formatTokens(c.input_tokens)}</td>
                          <td style={{ padding: '7px 6px', textAlign: 'right', color: 'var(--text-secondary)' }}>{formatTokens(c.output_tokens)}</td>
                          <td style={{ padding: '7px 0 7px 6px', textAlign: 'right', fontWeight: 500 }}>{formatCost(c.cost_usd)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                {usage.more > 0 && (
                  <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>+ {formatCount(usage.more)} more calls</span>
                )}
              </div>

              <div style={{ paddingTop: '12px', borderTop: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: '6px' }}>
                {usage.link ? (
                  <Link to={usage.link.path} className="btn btn-primary" style={{ alignSelf: 'flex-start' }}>
                    {LINK_LABELS[usage.link.kind]}
                  </Link>
                ) : (
                  <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                    {noLinkSentence(usage.no_link_reason, row.detail)}
                  </span>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </SlideOver>
  )
}
