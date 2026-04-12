import { useEffect, useState } from 'react'
import { useParams } from 'react-router'
import { getAgentUsage } from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'
import type { AgentUsageStats } from '../interfaces/types'
import { UsageBarChart, ChannelTypeStackedChart, type UsageMetric, type DisplayMode } from '../components/UsageBarChart'
import { FiChevronDown, FiChevronRight, FiCalendar } from 'react-icons/fi'

const DATE_RANGE_OPTIONS = [
  { label: 'Last 7 days', value: 7 },
  { label: 'Last 14 days', value: 14 },
  { label: 'Last 30 days', value: 30 },
  { label: 'Last 90 days', value: 90 },
]

const METRIC_OPTIONS: { label: string; value: UsageMetric }[] = [
  { label: 'Total', value: 'total' },
  { label: 'Input', value: 'input' },
  { label: 'Output', value: 'output' },
]

const DISPLAY_MODE_OPTIONS: { label: string; value: DisplayMode }[] = [
  { label: 'Grouped', value: 'grouped' },
  { label: 'Stacked', value: 'stacked' },
]

const CHANNEL_COLORS: Record<string, string> = {
  http: '#3b82f6',
  discord: '#5865f2',
  telegram: '#0088cc',
  task: '#f59e0b',
  inter_agent: '#8b5cf6',
  heartbeat: '#ef4444',
  system: '#6b7280',
  subagent: '#14b8a6',
  other: '#9ca3af',
}

function formatNumber(num: number): string {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  }
  if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toString()
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
  return `${(ms / 60000).toFixed(1)}m`
}

export default function UsageDashboard() {
  const { agentId } = useParams()
  const { addToast } = useToastStore()

  const [usage, setUsage] = useState<AgentUsageStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [dateRange, setDateRange] = useState(7)
  const [showDatePicker, setShowDatePicker] = useState(false)
  const [customStartDate, setCustomStartDate] = useState('')
  const [customEndDate, setCustomEndDate] = useState('')

  const [dayMetric, setDayMetric] = useState<UsageMetric>('total')
  const [displayMode, setDisplayMode] = useState<DisplayMode>('grouped')

  const [expandedChannels, setExpandedChannels] = useState<Record<string, boolean>>({})

  useEffect(() => {
    loadUsage()
  }, [agentId, dateRange, customStartDate, customEndDate])

  const getDateRange = () => {
    const end = new Date()
    const start = new Date()

    if (customStartDate && customEndDate) {
      return {
        startDate: new Date(customStartDate).toISOString(),
        endDate: new Date(customEndDate).toISOString(),
      }
    }

    start.setDate(start.getDate() - dateRange)
    return {
      startDate: start.toISOString(),
      endDate: end.toISOString(),
    }
  }

  const loadUsage = async () => {
    if (!agentId) return

    try {
      setLoading(true)
      const { startDate, endDate } = getDateRange()
      const response = await getAgentUsage(agentId, startDate, endDate)
      setUsage(response.data)

      const initialExpanded: Record<string, boolean> = {}
      if (response.data?.by_channel_type) {
        Object.keys(response.data.by_channel_type).forEach((key) => {
          initialExpanded[key] = true
        })
      }
      setExpandedChannels(initialExpanded)
    } catch (error) {
      console.error('Failed to load usage:', error)
      addToast('error', 'Failed to load usage', 'Please try again')
    } finally {
      setLoading(false)
    }
  }

  const toggleChannelType = (channelType: string) => {
    setExpandedChannels((prev) => ({
      ...prev,
      [channelType]: !prev[channelType],
    }))
  }

  const getDayChartData = () => {
    if (!usage?.by_day) return []
    return usage.by_day.map((day) => ({
      name: new Date(day.date).toLocaleDateString('en-US', { month: 'short', day: 'numeric' }),
      total: day.total_tokens,
      input: day.input_tokens,
      output: day.output_tokens,
    }))
  }

  const getDayAndChannelTypeChartData = () => {
    if (!usage?.by_day_and_channel_type) return []
    const channelTypes = new Set<string>()
    usage.by_day_and_channel_type.forEach((day) => {
      Object.keys(day.by_channel_type).forEach((ct) => channelTypes.add(ct))
    })
    const sortedChannelTypes = Array.from(channelTypes).sort()
    return usage.by_day_and_channel_type.map((day) => {
      const entry: Record<string, string | number> = {
        name: new Date(day.date).toLocaleDateString('en-US', { month: 'short', day: 'numeric' }),
      }
      sortedChannelTypes.forEach((ct) => {
        entry[ct] = day.by_channel_type[ct]?.total_tokens || 0
      })
      return entry
    })
  }

  const getChannelTypes = (): string[] => {
    if (!usage?.by_day_and_channel_type) return []
    const channelTypes = new Set<string>()
    usage.by_day_and_channel_type.forEach((day) => {
      Object.keys(day.by_channel_type).forEach((ct) => channelTypes.add(ct))
    })
    return Array.from(channelTypes).sort()
  }

  const formatDateRange = () => {
    if (customStartDate && customEndDate) {
      return `${customStartDate} - ${customEndDate}`
    }
    return `Last ${dateRange} days`
  }

  if (loading) {
    return (
      <div style={{ padding: '24px' }}>
        <div style={{ color: 'var(--text-tertiary)' }}>Loading usage data...</div>
      </div>
    )
  }

  if (!usage) {
    return (
      <div style={{ padding: '24px' }}>
        <div style={{ color: 'var(--text-tertiary)' }}>No usage data available</div>
      </div>
    )
  }

  return (
    <>
      {/* Header */}
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0 }}>Usage Analytics</h3>
        </div>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <div style={{ position: 'relative' }}>
            <button
              onClick={() => setShowDatePicker(!showDatePicker)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                padding: '8px 12px',
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: '6px',
                cursor: 'pointer',
                color: 'var(--text-primary)',
                fontSize: '14px',
              }}
            >
              <FiCalendar size={16} />
              {formatDateRange()}
            </button>

            {showDatePicker && (
              <div
                style={{
                  position: 'absolute',
                  top: '100%',
                  right: 0,
                  marginTop: '8px',
                  background: 'var(--surface)',
                  border: '1px solid var(--border)',
                  borderRadius: '8px',
                  padding: '16px',
                  zIndex: 100,
                  minWidth: '280px',
                }}
              >
                <div style={{ marginBottom: '12px' }}>
                  <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '8px' }}>
                    Quick Select
                  </div>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
                    {DATE_RANGE_OPTIONS.map((option) => (
                      <button
                        key={option.value}
                        onClick={() => {
                          setDateRange(option.value)
                          setCustomStartDate('')
                          setCustomEndDate('')
                          setShowDatePicker(false)
                        }}
                        style={{
                          padding: '4px 8px',
                          fontSize: '12px',
                          background: dateRange === option.value ? 'var(--accent-primary)' : 'var(--bg-secondary)',
                          color: dateRange === option.value ? 'white' : 'var(--text-primary)',
                          border: 'none',
                          borderRadius: '4px',
                          cursor: 'pointer',
                        }}
                      >
                        {option.label}
                      </button>
                    ))}
                  </div>
                </div>

                <div>
                  <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '8px' }}>
                    Custom Range
                  </div>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <input
                      type="date"
                      value={customStartDate}
                      onChange={(e) => setCustomStartDate(e.target.value)}
                      style={{
                        padding: '4px 8px',
                        fontSize: '12px',
                        background: 'var(--bg-secondary)',
                        border: '1px solid var(--border)',
                        borderRadius: '4px',
                        color: 'var(--text-primary)',
                      }}
                    />
                    <input
                      type="date"
                      value={customEndDate}
                      onChange={(e) => setCustomEndDate(e.target.value)}
                      style={{
                        padding: '4px 8px',
                        fontSize: '12px',
                        background: 'var(--bg-secondary)',
                        border: '1px solid var(--border)',
                        borderRadius: '4px',
                        color: 'var(--text-primary)',
                      }}
                    />
                  </div>
                  {(customStartDate || customEndDate) && (
                    <button
                      onClick={() => {
                        setCustomStartDate('')
                        setCustomEndDate('')
                        setShowDatePicker(false)
                      }}
                      style={{
                        marginTop: '8px',
                        padding: '4px 8px',
                        fontSize: '12px',
                        background: 'transparent',
                        border: 'none',
                        color: 'var(--accent-primary)',
                        cursor: 'pointer',
                      }}
                    >
                      Clear custom range
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>

        </div>

      </div>

      <div style={{ padding: '24px', maxWidth: '100%', }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px' }}>
          <h1 style={{ fontSize: '24px', fontWeight: 600 }}>Usage Dashboard</h1>

          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}></div>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '16px', marginBottom: '32px' }}>
          <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '4px' }}>
              Total Tokens
            </div>
            <div style={{ fontSize: '24px', fontWeight: 600, color: 'var(--accent-primary)' }}>
              {formatNumber(usage.summary.total_tokens)}
            </div>
          </div>
          <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '4px' }}>
              Input Tokens
            </div>
            <div style={{ fontSize: '24px', fontWeight: 600 }}>
              {formatNumber(usage.summary.total_input_tokens)}
            </div>
          </div>
          <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '4px' }}>
              Output Tokens
            </div>
            <div style={{ fontSize: '24px', fontWeight: 600 }}>
              {formatNumber(usage.summary.total_output_tokens)}
            </div>
          </div>
          <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '4px' }}>
              Total Requests
            </div>
            <div style={{ fontSize: '24px', fontWeight: 600 }}>
              {formatNumber(usage.summary.total_requests)}
            </div>
          </div>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: '16px', marginBottom: '24px' }}>
          <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h2 style={{ fontSize: '16px', fontWeight: 600 }}>Token Usage by Day (Input/Output)</h2>
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <select
                  value={dayMetric}
                  onChange={(e) => setDayMetric(e.target.value as UsageMetric)}
                  style={{
                    padding: '4px 8px',
                    fontSize: '12px',
                    background: 'var(--bg-secondary)',
                    border: '1px solid var(--border)',
                    borderRadius: '4px',
                    color: 'var(--text-primary)',
                    cursor: 'pointer',
                  }}
                >
                  {METRIC_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
                <select
                  value={displayMode}
                  onChange={(e) => setDisplayMode(e.target.value as DisplayMode)}
                  style={{
                    padding: '4px 8px',
                    fontSize: '12px',
                    background: 'var(--bg-secondary)',
                    border: '1px solid var(--border)',
                    borderRadius: '4px',
                    color: 'var(--text-primary)',
                    cursor: 'pointer',
                  }}
                >
                  {DISPLAY_MODE_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <UsageBarChart data={getDayChartData()} metric={dayMetric} displayMode={displayMode} />
          </div>

          <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h2 style={{ fontSize: '16px', fontWeight: 600 }}>Token Usage by Day (Channel Type)</h2>
            </div>
            {usage.by_day_and_channel_type && usage.by_day_and_channel_type.length > 0 ? (
              <ChannelTypeStackedChart
                data={getDayAndChannelTypeChartData()}
                channelTypes={getChannelTypes()}
                colors={CHANNEL_COLORS}
              />
            ) : (
              <div style={{ color: 'var(--text-tertiary)', textAlign: 'center', padding: '80px 0' }}>
                No channel type data available
              </div>
            )}
          </div>
        </div>

        <div style={{ background: 'var(--surface)', borderRadius: '8px', padding: '16px' }}>
          <h2 style={{ fontSize: '16px', fontWeight: 600, marginBottom: '16px' }}>Channels</h2>

          {Object.keys(usage.by_channel_type || {}).length === 0 ? (
            <div style={{ color: 'var(--text-tertiary)', textAlign: 'center', padding: '32px' }}>
              No channel data available
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {Object.entries(usage.by_channel_type).map(([channelType, data]) => (
                <div
                  key={channelType}
                  style={{
                    border: '1px solid var(--border)',
                    borderRadius: '6px',
                    overflow: 'hidden',
                  }}
                >
                  <div
                    onClick={() => toggleChannelType(channelType)}
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      padding: '12px 16px',
                      cursor: 'pointer',
                      background: 'var(--bg-secondary)',
                      userSelect: 'none',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      {expandedChannels[channelType] ? <FiChevronDown size={16} /> : <FiChevronRight size={16} />}
                      <span style={{ fontWeight: 500, textTransform: 'capitalize' }}>{channelType}</span>
                      <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                        ({data.channels.length} channels)
                      </span>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                      <span style={{ fontSize: '14px', fontWeight: 500 }}>
                        {formatNumber(data.total_tokens)} tokens
                      </span>
                      <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                        {data.total_requests} requests
                      </span>
                      <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                        avg {formatDuration(data.total_tokens / data.total_requests)}/req
                      </span>
                    </div>
                  </div>

                  {expandedChannels[channelType] && (
                    <div style={{ padding: '8px 16px 8px 32px' }}>
                      {data.channels.map((channel) => (
                        <div
                          key={channel.channel_id}
                          style={{
                            display: 'flex',
                            justifyContent: 'space-between',
                            alignItems: 'center',
                            padding: '8px 0',
                            borderBottom: '1px solid var(--border)',
                          }}
                        >
                          <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                            {channel.channel_id.replace(/__/g, ' / ')}
                          </span>
                          <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                            <span style={{ fontSize: '13px' }}>
                              {formatNumber(channel.total_tokens)} tokens
                            </span>
                            <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                              {channel.total_requests} requests
                            </span>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

    </>
  )
}
