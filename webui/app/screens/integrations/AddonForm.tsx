import { Button, Dialog, Select, SwitchRow, Textarea, TextField, ToolList } from '@vizier/design'
import { useState } from 'react'
import { ApiError } from '../../api/client'
import type { IntegrationItem, Kind, KindField } from '../../api/endpoints/integrations'
import { ADDON_TOOLS } from '../../lib/integrations'

export type SaveAddon = (kind: Kind, settings: Record<string, unknown>) => Promise<void>

/** Install a new addon, or edit the settings of one already on the agent. */
export function AddonDialog({
    kind,
    mode,
    item,
    onClose,
    onSave,
    onRemove,
}: {
    kind: Kind | null
    mode: 'install' | 'manage'
    item?: IntegrationItem
    onClose: () => void
    onSave: SaveAddon
    onRemove?: () => void
}) {
    return (
        <Dialog
            open={kind !== null}
            title={mode === 'install' ? `Add ${kind?.name ?? 'addon'}` : `${kind?.name ?? 'Addon'} settings`}
            size="lg"
            confirmLabel=""
            onConfirm={() => {}}
            onCancel={onClose}
        >
            {kind && <AddonForm key={`${mode}-${item?.id ?? kind.id}`} kind={kind} mode={mode} item={item} onClose={onClose} onSave={onSave} onRemove={onRemove} />}
        </Dialog>
    )
}

function AddonForm({ kind, mode, item, onClose, onSave, onRemove }: { kind: Kind; mode: 'install' | 'manage'; item?: IntegrationItem; onClose: () => void; onSave: SaveAddon; onRemove?: () => void }) {
    const [values, setValues] = useState<Record<string, unknown>>(() => {
        if (mode === 'manage' && item) {
            // Secrets stay out of the fields: the masked value is the placeholder, and an empty
            // field means "keep it".
            const next: Record<string, unknown> = { ...item.settings }
            for (const field of kind.fields) if (field.type === 'secret') next[field.key] = ''
            return next
        }
        return Object.fromEntries(kind.fields.filter((f) => f.default !== undefined).map((f) => [f.key, f.default]))
    })
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()
    const ready = kind.fields.every((f) => !f.required || String(values[f.key] ?? '').trim() !== '')
    const tools = ADDON_TOOLS[kind.id]

    // A secret left blank means "keep the stored one": it is left out of the request.
    const settings = Object.fromEntries(
        kind.fields.flatMap((f) => {
            const value = values[f.key]
            if (f.type === 'secret' && !String(value ?? '').trim()) return []
            return [[f.key, value]]
        })
    )

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setProblem(undefined)
                try {
                    await onSave(kind, settings)
                } catch (err) {
                    setProblem(err instanceof ApiError || err instanceof Error ? err.message : mode === 'install' ? 'It could not be added.' : 'It could not be saved.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            <p className="-mt-4 type-ui-para text-muted">{kind.description}</p>
            {kind.fields.length === 0 ? (
                <p className="rounded-lg border border-border bg-ground px-16 py-12 type-small text-muted">This addon needs nothing else. Its tools are ready the moment it is added.</p>
            ) : (
                <div className="flex flex-col gap-16">
                    {kind.fields.map((f) => (
                        <AddonField
                            key={f.key}
                            field={f}
                            value={values[f.key]}
                            stored={mode === 'manage' ? item?.settings[f.key] : undefined}
                            onChange={(value) => setValues((s) => ({ ...s, [f.key]: value }))}
                        />
                    ))}
                </div>
            )}
            {tools && (
                <div className="flex flex-col gap-8">
                    <span className="type-ui-medium text-soft">Its tools</span>
                    <ToolList tools={tools} />
                </div>
            )}
            {problem && <p className="type-small text-failed-text">{problem}</p>}
            <div className="flex flex-wrap items-center justify-end gap-8 pt-4">
                {mode === 'manage' && onRemove && (
                    <Button type="button" variant="destructive" icon="trash" className="mr-auto" onClick={onRemove}>
                        Remove
                    </Button>
                )}
                {kind.fields.length > 0 && (
                    <Button type="button" variant="ghost" onClick={onClose}>
                        Cancel
                    </Button>
                )}
                {mode === 'manage' && kind.fields.length === 0 ? (
                    <Button type="button" variant="primary" onClick={onClose}>
                        Done
                    </Button>
                ) : (
                    <Button type="submit" variant="primary" loading={busy} disabled={!ready}>
                        {mode === 'install' ? 'Add addon' : 'Save'}
                    </Button>
                )}
            </div>
        </form>
    )
}

function AddonField({ field, value, stored, onChange }: { field: KindField; value: unknown; stored: unknown; onChange: (value: unknown) => void }) {
    if (field.type === 'bool') {
        return <SwitchRow label={field.label} checked={Boolean(value ?? field.default)} onCheckedChange={onChange} />
    }
    if (field.type === 'select' && field.options?.length) {
        return (
            <Select
                label={field.label}
                value={String(value ?? field.default ?? '')}
                options={(field.options ?? []).map((option) => ({ value: option, label: option }))}
                onChange={onChange}
            />
        )
    }
    if (field.key === 'env' || field.type === 'secret_map' || field.type === 'string_list') {
        return (
            <Textarea
                label={field.label}
                mono
                rows={4}
                value={String(value ?? '')}
                placeholder={stored ? String(stored) : undefined}
                helper={
                    stored
                        ? 'One KEY=value per line. Blank lines and # comments are skipped. Leave empty to keep the saved values.'
                        : 'One KEY=value per line. Blank lines and # comments are skipped.'
                }
                onChange={(e) => onChange(e.target.value)}
            />
        )
    }
    if (field.type === 'secret') {
        return (
            <TextField
                label={field.label}
                mono
                type="password"
                autoComplete="off"
                value={String(value ?? '')}
                placeholder={stored ? String(stored) : undefined}
                helper={stored ? 'Leave empty to keep the saved value.' : undefined}
                onChange={(e) => onChange(e.target.value)}
            />
        )
    }
    return (
        <TextField
            label={field.label}
            mono={field.required}
            value={String(value ?? '')}
            helper={field.key === 'path' ? 'An absolute path on the machine running Vizier.' : undefined}
            onChange={(e) => onChange(e.target.value)}
        />
    )
}
