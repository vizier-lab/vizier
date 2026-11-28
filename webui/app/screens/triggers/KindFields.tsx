import { Button, CodeEditor, Select, SwitchRow, Textarea, TextField } from '@vizier/design'
import { useState } from 'react'
import type { KindField } from '../../api/endpoints/triggers'
import { highlightCode } from '../../lib/highlight'
import type { FieldValues } from '../../lib/triggers'
import { CategoryChips } from './ToolAccess'

type Props = {
    fields: KindField[]
    values: FieldValues
    /** The saved settings, whose masked secrets show as placeholders. */
    saved?: Record<string, unknown>
    onChange: (key: string, value: string | boolean | string[]) => void
}

/** A kind's own settings, drawn from what its listing says of each field. */
export function KindFields({ fields, ...rest }: Props) {
    const shown = fields.filter((f) => !f.generated)
    const basic = shown.filter((f) => !f.advanced)
    const advanced = shown.filter((f) => f.advanced)
    const [open, setOpen] = useState(false)
    return (
        <div className="flex flex-col gap-16">
            {basic.map((f) => (
                <Field key={f.key} field={f} {...rest} />
            ))}
            {advanced.length > 0 && (
                <>
                    <Button type="button" variant="ghost" size="sm" icon={open ? 'chevron-down' : 'chevron-right'} className="self-start" aria-expanded={open} onClick={() => setOpen(!open)}>
                        Optional checks
                    </Button>
                    {open && advanced.map((f) => <Field key={f.key} field={f} {...rest} />)}
                </>
            )}
        </div>
    )
}

function Field({ field, values, saved, onChange }: { field: KindField } & Omit<Props, 'fields'>) {
    const value = values[field.key]
    const text = typeof value === 'string' ? value : ''
    const set = (v: string | boolean | string[]) => onChange(field.key, v)
    switch (field.type) {
        case 'bool':
            return <SwitchRow label={field.label} checked={value === true} onCheckedChange={set} />
        case 'select':
            return <Select label={field.label} value={text} options={(field.options ?? []).map((o) => ({ value: o, label: o }))} onChange={set} />
        case 'code':
            return <CodeEditor label={field.label} lineNumbers rows={10} value={text} tokens={highlightCode(text, 'python')} onChange={set} />
        case 'tool_categories':
            return (
                <div className="flex flex-col gap-8">
                    <span className="type-ui-medium text-soft">{field.label}</span>
                    <CategoryChips forScript selected={Array.isArray(value) ? value : []} onChange={set} />
                </div>
            )
        case 'string_list':
            return <Textarea label={field.label} mono rows={3} value={text} helper="One per line." onChange={(e) => set(e.target.value)} />
        case 'secret': {
            const stored = saved?.[field.key]
            return (
                <TextField
                    label={field.label}
                    mono
                    type="password"
                    autoComplete="off"
                    value={text}
                    placeholder={stored ? String(stored) : undefined}
                    helper={stored ? 'Leave empty to keep the saved value.' : undefined}
                    onChange={(e) => set(e.target.value)}
                />
            )
        }
        case 'number':
            return <TextField label={field.label} mono inputMode="numeric" value={text} onChange={(e) => set(e.target.value)} />
        default:
            return <TextField label={field.label} mono value={text} onChange={(e) => set(e.target.value)} />
    }
}
