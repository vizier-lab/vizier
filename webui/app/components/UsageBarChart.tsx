import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from 'recharts'

export type UsageMetric = 'total' | 'input' | 'output'
export type DisplayMode = 'grouped' | 'stacked'

interface UsageBarChartProps {
  data: Array<{
    name: string
    total: number
    input: number
    output: number
  }>
  metric: UsageMetric
  displayMode?: DisplayMode
}

const CustomTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    return (
      <div
        style={{
          background: 'var(--surface)',
          border: '1px solid var(--border)',
          borderRadius: '4px',
          padding: '8px 12px',
          fontSize: '12px',
        }}
      >
        <p style={{ fontWeight: 600, marginBottom: '4px' }}>{label}</p>
        {payload.map((entry: any, index: number) => (
          <p key={index} style={{ color: entry.color }}>
            {entry.name}: {entry.value?.toLocaleString() || 0}
          </p>
        ))}
      </div>
    )
  }
  return null
}

export function UsageBarChart({ data, metric, displayMode = 'grouped' }: UsageBarChartProps) {
  const isStacked = displayMode === 'stacked'

  return (
    <div style={{ width: '100%', height: 300 }}>
      <ResponsiveContainer>
        <BarChart
          data={data}
          margin={{ top: 20, right: 30, left: 20, bottom: 5 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
          <XAxis
            dataKey="name"
            tick={{ fill: 'var(--text-secondary)', fontSize: 11 }}
            axisLine={{ stroke: 'var(--border)' }}
          />
          <YAxis
            tick={{ fill: 'var(--text-secondary)', fontSize: 11 }}
            axisLine={{ stroke: 'var(--border)' }}
            tickFormatter={(value) => value.toLocaleString()}
          />
          <Tooltip content={<CustomTooltip />} />
          {isStacked ? (
            <>
              <Bar
                dataKey="input"
                stackId="stack"
                fill="#14b8a6"
                radius={[0, 0, 0, 0]}
                maxBarSize={50}
                name="Input"
              />
              <Bar
                dataKey="output"
                stackId="stack"
                fill="#f97316"
                radius={[4, 4, 0, 0]}
                maxBarSize={50}
                name="Output"
              />
            </>
          ) : (
            <Bar
              dataKey={metric}
              fill="var(--accent-primary)"
              radius={[4, 4, 0, 0]}
              maxBarSize={50}
            />
          )}
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}

interface ChannelTypeStackedChartProps {
  data: Array<Record<string, string | number>>
  channelTypes: string[]
  colors: Record<string, string>
}

export function ChannelTypeStackedChart({ data, channelTypes, colors }: ChannelTypeStackedChartProps) {
  return (
    <div style={{ width: '100%', height: 300 }}>
      <ResponsiveContainer>
        <BarChart
          data={data}
          margin={{ top: 20, right: 30, left: 20, bottom: 5 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
          <XAxis
            dataKey="name"
            tick={{ fill: 'var(--text-secondary)', fontSize: 11 }}
            axisLine={{ stroke: 'var(--border)' }}
          />
          <YAxis
            tick={{ fill: 'var(--text-secondary)', fontSize: 11 }}
            axisLine={{ stroke: 'var(--border)' }}
            tickFormatter={(value) => value.toLocaleString()}
          />
          <Tooltip content={<CustomTooltip />} />
          {channelTypes.map((channelType) => (
            <Bar
              key={channelType}
              dataKey={channelType}
              stackId="stack"
              fill={colors[channelType] || '#6b7280'}
              radius={[0, 0, 0, 0]}
              maxBarSize={50}
              name={channelType}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}

export function ChannelTypeBarChart({ data, metric }: Omit<UsageBarChartProps, 'displayMode'>) {
  return (
    <div style={{ width: '100%', height: 300 }}>
      <ResponsiveContainer>
        <BarChart
          data={data}
          layout="horizontal"
          margin={{ top: 20, right: 30, left: 0, bottom: 5 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
          <XAxis
            dataKey="name"
            tick={{ fill: 'var(--text-secondary)', fontSize: 11 }}
            axisLine={{ stroke: 'var(--border)' }}
          />
          <YAxis
            tick={{ fill: 'var(--text-secondary)', fontSize: 11 }}
            axisLine={{ stroke: 'var(--border)' }}
            width={70}
          />
          <Tooltip content={<CustomTooltip />} />
          <Bar
            dataKey={metric}
            fill="var(--accent-primary)"
            radius={[0, 4, 4, 0]}
            maxBarSize={30}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}