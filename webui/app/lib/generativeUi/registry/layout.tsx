import { Card, Grid, Row, Stack, Tab, TabList } from '@vizier/design'
import { useState } from 'react'
import type { ComponentRenderProps } from '@json-render/react'
import { useEmitter, useField, gapOf } from './shared'

type P = Record<string, unknown>

export function StackC({ element, children }: ComponentRenderProps<P>) {
    const p = element.props
    return (
        <Stack gap={gapOf(p.gap)} align={(p.align as 'start' | 'center' | 'end' | 'stretch') ?? 'stretch'}>
            {children}
        </Stack>
    )
}

export function RowC({ element, children }: ComponentRenderProps<P>) {
    const p = element.props
    return (
        <Row
            gap={gapOf(p.gap)}
            align={(p.align as 'start' | 'center' | 'end' | 'stretch') ?? 'center'}
            justify={(p.justify as 'start' | 'center' | 'end' | 'between') ?? 'start'}
            wrap={p.wrap === true}
        >
            {children}
        </Row>
    )
}

export function GridC({ element, children }: ComponentRenderProps<P>) {
    const p = element.props
    const grid = (
        <Grid cols={typeof p.cols === 'number' ? p.cols : 2} gap={gapOf(p.gap)} minCol={typeof p.minCol === 'number' ? p.minCol : undefined}>
            {children}
        </Grid>
    )
    return typeof p.maxWidth === 'number' ? <div style={{ maxWidth: p.maxWidth, width: '100%', marginInline: 'auto' }}>{grid}</div> : grid
}

export function CardC({ element, children }: ComponentRenderProps<P>) {
    const p = element.props
    return (
        <Card
            level={3}
            title={typeof p.title === 'string' ? p.title : undefined}
            description={typeof p.subtitle === 'string' ? p.subtitle : undefined}
            tone={p.tone === 'error' ? 'danger' : 'default'}
        >
            {children}
        </Card>
    )
}

export function TabsC({ element, children, bindings, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const tabs = (Array.isArray(p.tabs) ? p.tabs : []) as { id: string; label: string }[]
    const [value, setValue] = useField<string>(p.value as string | undefined, bindings?.value, tabs[0]?.id ?? '')
    const send = useEmitter(emit)
    return (
        <div className="flex min-w-0 flex-col gap-12">
            <TabList aria-label={typeof p.label === 'string' ? p.label : 'Sections'}>
                {tabs.map((tab) => (
                    <Tab
                        key={tab.id}
                        selected={tab.id === value}
                        onClick={(e) => {
                            setValue(tab.id)
                            send('change', e.nativeEvent, tab.id)
                        }}
                    >
                        {tab.label}
                    </Tab>
                ))}
            </TabList>
            {children}
        </div>
    )
}

export function FoldC({ element, children }: ComponentRenderProps<P>) {
    const p = element.props
    const [open, setOpen] = useState(p.open === true)
    return (
        <div className="flex min-w-0 flex-col gap-8">
            <button
                type="button"
                aria-expanded={open}
                onClick={() => setOpen((o) => !o)}
                className="flex items-center gap-6 self-start rounded-xs type-ui-medium text-soft hover:text-fg"
            >
                <span aria-hidden className="type-mono-xs text-faint">
                    {open ? '▾' : '▸'}
                </span>
                {String(p.title ?? '')}
            </button>
            {open && children}
        </div>
    )
}

export function DividerC() {
    return <hr className="border-border" />
}
