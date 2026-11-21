// The `<vz-*>` elements (specs/063-html-generative-ui tasks T052), one per entry of
// `../manifest.ts`, each drawing the `@vizier/design` component the json-render catalogue used for
// the same job, so a page built from them looks like the rest of Vizier.

import {
    Badge,
    Callout as Hint,
    Card,
    Chart,
    Checkbox,
    Notice,
    ProgressBar,
    Select,
    Slider,
    StatGroup,
    Switch,
    Tab,
    Table,
    TableRow,
    TabList,
    TextField,
    Textarea,
} from '@vizier/design'
import { useEffect, useLayoutEffect, useState } from 'react'
import { ELEMENTS } from '../manifest'
import { defineElement, type RenderContext, type VzElement } from './define'
import { type FieldValue, formatValue, initialValue } from './values'

const spec = (name: string) => {
    const found = ELEMENTS.find((e) => e.name === name)
    if (!found) throw new Error(`no spec for ${name}`)
    return found
}

const str = (v: unknown) => (typeof v === 'string' ? v : undefined)
const num = (v: unknown) => (typeof v === 'number' ? v : undefined)

const BADGE = { default: 'neutral', accent: 'rank', ok: 'live', warn: 'retrying', error: 'failed' } as const

// Layout and text ------------------------------------------------------------------------------

defineElement(spec('vz-card'), ({ values, body }) => (
    <Card level={3} title={str(values.title)} description={str(values.subtitle)} tone={values.tone === 'error' ? 'danger' : 'default'}>
        {body}
    </Card>
))

defineElement(spec('vz-badge'), ({ values, body }) => (
    <Badge tone={BADGE[(values.tone as keyof typeof BADGE) ?? 'default'] ?? 'neutral'}>{body}</Badge>
))

defineElement(spec('vz-callout'), ({ values, body }) => {
    const title = str(values.title)
    if (values.tone === 'warn' || values.tone === 'error')
        return (
            <Notice tone={values.tone === 'warn' ? 'retrying' : 'failed'} title={title}>
                {body}
            </Notice>
        )
    return <Hint title={title}>{body}</Hint>
})

function Tabs({ element, values, body, emit }: RenderContext) {
    const sections = () => [...element.body.children].filter((c): c is HTMLElement => c instanceof HTMLElement && c.tagName === 'SECTION')
    const labels = sections().map((s, i) => s.getAttribute('label') ?? `Tab ${i + 1}`)
    const [value, setValue] = useState(() => (str(values.value) && labels.includes(str(values.value)!) ? str(values.value)! : labels[0]))
    useLayoutEffect(() => {
        sections().forEach((s, i) => {
            s.hidden = labels[i] !== value
        })
    })
    return (
        <div className="flex min-w-0 flex-col gap-12">
            <TabList aria-label={str(values.label) ?? 'Sections'}>
                {labels.map((label) => (
                    <Tab
                        key={label}
                        selected={label === value}
                        onClick={() => {
                            setValue(label)
                            emit('change', { value: label })
                        }}
                    >
                        {label}
                    </Tab>
                ))}
            </TabList>
            {body}
        </div>
    )
}
defineElement(spec('vz-tabs'), (ctx) => <Tabs {...ctx} />)

// Data ------------------------------------------------------------------------------------------

defineElement(spec('vz-stat'), ({ values }) =>
    values.value === undefined ? null : (
        <StatGroup
            stats={[
                {
                    label: str(values.label) ?? '',
                    value: formatValue(values.value, values.format),
                    sub: str(values.delta),
                    tone: values.tone === 'error' ? 'failed' : 'default',
                },
            ]}
        />
    )
)

type Column = { key: string; label: string; align?: 'start' | 'end'; format?: string }

defineElement(spec('vz-table'), ({ values, emit }) => {
    const columns = (values.columns as Column[] | undefined) ?? []
    const rows = (values.rows as Record<string, unknown>[] | undefined) ?? []
    const limit = num(values['max-rows']) ?? 200
    if (columns.length === 0) return null
    return (
        <Table
            label={str(values.label) ?? 'Table'}
            columns={columns.map((c) => ({ label: c.label, align: c.align === 'end' ? 'end' : 'start' }))}
            footer={rows.length > limit ? <p className="px-16 py-8 type-small text-faint">Showing {limit} of {rows.length} rows</p> : undefined}
        >
            {rows.slice(0, limit).map((row, index) => (
                <TableRow
                    key={index}
                    cells={columns.map((c) => formatValue(row?.[c.key], c.format))}
                    endAligned={columns.flatMap((c, i) => (c.align === 'end' ? [i] : []))}
                    onSelect={values.selectable === true ? () => emit('select', { row, index }) : undefined}
                />
            ))}
        </Table>
    )
})

defineElement(spec('vz-chart'), ({ values, emit }) =>
    values.kind === undefined || values.data === undefined ? null : (
        <Chart
            kind={values.kind as 'bar' | 'line' | 'area' | 'pie'}
            data={values.data as { label: string; value: number; series?: string }[]}
            legend={typeof values.legend === 'boolean' ? values.legend : undefined}
            height={num(values.height)}
            title={str(values.title)}
            onPress={(datum) => emit('press', { label: datum.label, value: datum.value })}
        />
    )
)

defineElement(spec('vz-progress'), ({ values }) => {
    const max = num(values.max) ?? 1
    const fraction = Math.min(1, Math.max(0, (num(values.value) ?? 0) / (max || 1)))
    return <ProgressBar label={str(values.label) ?? ''} value={fraction * 100} state={values.tone === 'error' ? 'failed' : fraction >= 1 ? 'done' : 'running'} />
})

// Inputs ------------------------------------------------------------------------------------------

const values = new WeakMap<VzElement, FieldValue>()

function Field({ element, values: v, emit }: RenderContext) {
    const kind = str(v.kind) ?? 'text'
    const [value, setValue] = useState<FieldValue>(() => initialValue(kind, str(v.value), num(v.min)))
    useEffect(() => {
        values.set(element, value)
    }, [element, value])
    values.set(element, value)
    const name = str(v.name) ?? ''
    const label = str(v.label) ?? ''
    const disabled = v.disabled === true
    const helper = str(v.help)
    const change = (next: FieldValue, events: ('input' | 'change')[]) => {
        setValue(next)
        values.set(element, next)
        for (const e of events) emit(e, { name, value: next })
    }
    switch (kind) {
        case 'textarea':
            return (
                <Textarea
                    label={label}
                    value={String(value)}
                    disabled={disabled}
                    placeholder={str(v.placeholder)}
                    helper={helper}
                    onChange={(e) => change(e.target.value, ['input'])}
                    onBlur={() => emit('change', { name, value: values.get(element) })}
                />
            )
        case 'number':
            return (
                <TextField
                    type="number"
                    label={label}
                    value={value as number | ''}
                    min={num(v.min)}
                    max={num(v.max)}
                    step={num(v.step)}
                    disabled={disabled}
                    placeholder={str(v.placeholder)}
                    helper={helper}
                    onChange={(e) => change(e.target.value === '' ? '' : Number(e.target.value), ['input'])}
                    onBlur={() => emit('change', { name, value: values.get(element) })}
                />
            )
        case 'select': {
            const options = ((v.options as (string | { value: string; label: string })[] | undefined) ?? []).map((o) =>
                typeof o === 'string' ? { value: o, label: o } : o
            )
            return (
                <Select
                    label={label}
                    options={options}
                    value={String(value) || undefined}
                    disabled={disabled}
                    helper={helper}
                    onChange={(next) => change(next, ['input', 'change'])}
                />
            )
        }
        case 'checkbox':
            return <Checkbox label={label} checked={value === true} disabled={disabled} onChange={(e) => change(e.target.checked, ['input', 'change'])} />
        case 'switch':
            return (
                <label className="inline-flex items-center gap-8 type-ui text-fg">
                    <Switch checked={value === true} disabled={disabled} aria-label={label} onCheckedChange={(next) => change(next, ['input', 'change'])} />
                    {label}
                </label>
            )
        case 'slider':
            return (
                <Slider
                    label={label}
                    value={typeof value === 'number' ? value : (num(v.min) ?? 0)}
                    min={num(v.min) ?? 0}
                    max={num(v.max) ?? 100}
                    step={num(v.step) ?? 1}
                    disabled={disabled}
                    onChange={(next) => change(next, ['input', 'change'])}
                />
            )
        default:
            return (
                <TextField
                    label={label}
                    value={String(value)}
                    disabled={disabled}
                    placeholder={str(v.placeholder)}
                    helper={helper}
                    onChange={(e) => change(e.target.value, ['input'])}
                    onBlur={() => emit('change', { name, value: values.get(element) })}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter') emit('change', { name, value: values.get(element) })
                    }}
                />
            )
    }
}

defineElement(spec('vz-field'), (ctx) => <Field {...ctx} />, (cls) => {
    Object.defineProperty(cls.prototype, 'value', {
        get(this: VzElement) {
            return values.get(this) ?? initialValue(this.getAttribute('kind') ?? 'text', this.getAttribute('value') ?? undefined, Number(this.getAttribute('min') ?? 0))
        },
    })
})

// Media -------------------------------------------------------------------------------------------

type Bridge = { files?: { url(name: string): Promise<string> } }

function Image({ values: v, emit }: RenderContext) {
    const file = str(v.file)
    const src = str(v.src)
    const [url, setUrl] = useState<string | null>(null)
    const [failed, setFailed] = useState<string | null>(null)
    useEffect(() => {
        let cancelled = false
        setFailed(null)
        if (file) {
            const bridge = (window as unknown as { vizier?: Bridge }).vizier
            if (!bridge?.files) {
                setFailed('files are not available here')
                return
            }
            bridge.files.url(file).then(
                (u) => !cancelled && setUrl(u),
                (e: Error) => {
                    if (cancelled) return
                    setFailed(e.message)
                    emit('error', { message: e.message })
                }
            )
        } else if (src && /^(https:|data:image\/)/.test(src)) {
            setUrl(src)
        } else {
            setFailed(src ? 'only https: and data: images load' : 'give a file or a src')
        }
        return () => {
            cancelled = true
        }
    }, [file, src])
    if (failed) return <span className="type-small text-failed-text">Image not shown: {failed}</span>
    if (!url) return <span aria-busy="true" style={{ display: 'inline-block', width: num(v.width) ?? 120, height: num(v.height) ?? 80 }} />
    return (
        <img
            src={url}
            alt={str(v.alt) ?? ''}
            width={num(v.width)}
            height={num(v.height)}
            className="max-w-full rounded-md border border-border"
            onLoad={() => emit('load', {})}
        />
    )
}
defineElement(spec('vz-image'), (ctx) => <Image {...ctx} />)
