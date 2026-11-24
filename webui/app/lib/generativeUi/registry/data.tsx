import { Chart, KeyValueList, ProgressBar, StatGroup, Table, TableRow } from '@vizier/design'
import type { ComponentRenderProps } from '@json-render/react'
import { useEmitter } from './shared'

type P = Record<string, unknown>

const MAX_DEFAULT_ROWS = 200

export function formatValue(value: unknown, format: unknown): string {
    if (value === null || value === undefined) return ''
    if (typeof value !== 'number') return String(value)
    switch (format) {
        case 'percent':
            return `${Math.round(value * 1000) / 10}%`
        case 'currency':
            return new Intl.NumberFormat(undefined, { style: 'currency', currency: 'USD' }).format(value)
        case 'bytes': {
            const units = ['B', 'KB', 'MB', 'GB', 'TB']
            let n = value
            let u = 0
            while (Math.abs(n) >= 1024 && u < units.length - 1) {
                n /= 1024
                u++
            }
            return `${Math.round(n * 10) / 10} ${units[u]}`
        }
        case 'duration': {
            const s = Math.round(value)
            return s >= 3600 ? `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m` : s >= 60 ? `${Math.floor(s / 60)}m ${s % 60}s` : `${s}s`
        }
        default:
            return new Intl.NumberFormat().format(value)
    }
}

export function StatC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    return (
        <StatGroup
            stats={[
                {
                    label: String(p.label ?? ''),
                    value: formatValue(p.value, p.format),
                    sub: p.delta === undefined ? undefined : String(p.delta),
                    tone: p.tone === 'error' ? 'failed' : 'default',
                },
            ]}
        />
    )
}

export function TableC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const columns = (Array.isArray(p.columns) ? p.columns : []) as { key: string; label: string; align?: string; format?: string }[]
    const rows = (Array.isArray(p.rows) ? p.rows : []) as Record<string, unknown>[]
    const limit = typeof p.maxRows === 'number' ? p.maxRows : MAX_DEFAULT_ROWS
    const send = useEmitter(emit)
    const selectable = p.selectable === true
    return (
        <Table
            label={String(p.label ?? 'Table')}
            columns={columns.map((c) => ({ label: c.label, align: c.align === 'end' ? 'end' : 'start' }))}
            footer={rows.length > limit ? <p className="px-16 py-8 type-small text-faint">Showing {limit} of {rows.length} rows</p> : undefined}
        >
            {rows.slice(0, limit).map((row, index) => (
                <TableRow
                    key={index}
                    cells={columns.map((c) => formatValue(row?.[c.key], c.format))}
                    endAligned={columns.flatMap((c, i) => (c.align === 'end' ? [i] : []))}
                    onSelect={selectable ? () => send('select', { isTrusted: true }, { row, index }) : undefined}
                />
            ))}
        </Table>
    )
}

export function KeyValueC({ element }: ComponentRenderProps<P>) {
    const items = (Array.isArray(element.props.items) ? element.props.items : []) as { label: string; value: unknown }[]
    return <KeyValueList items={items.map((i) => ({ label: i.label, value: String(i.value) }))} />
}

export function ProgressC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const value = typeof p.value === 'number' ? Math.min(1, Math.max(0, p.value)) : 0
    return <ProgressBar label={String(p.label ?? '')} value={value * 100} state={p.tone === 'error' ? 'failed' : value >= 1 ? 'done' : 'running'} />
}

export function ChartC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const send = useEmitter(emit)
    const data = (Array.isArray(p.data) ? p.data : []) as { label: string; value: number; series?: string }[]
    return (
        <Chart
            kind={p.kind as 'bar' | 'line' | 'area' | 'pie'}
            data={data}
            legend={typeof p.legend === 'boolean' ? p.legend : undefined}
            height={typeof p.height === 'number' ? p.height : undefined}
            title={typeof p.title === 'string' ? p.title : undefined}
            onPress={(datum) => send('press', { isTrusted: true }, { label: datum.label, value: datum.value })}
        />
    )
}
