import { Button, Callout, Card, Dialog, EmptyState, IntegrationCard, IntegrationLogo, SearchField, SecretField, Skeleton, SwitchRow, TextField, ToolList } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { ApiError } from '../../api/client'
import { createIntegration, integrationKinds, removeIntegration, updateIntegration, type IntegrationItem, type Kind, type KindField } from '../../api/endpoints/integrations'
import { ADDON_TOOLS, cardState, stateDetail } from '../../lib/integrations'
import { useIntegrationList } from '../../lib/useIntegrationList'
import { ActivityCard } from './ActivityCard'
import { SelfManagementCard } from './SelfManagementCard'
import { ConfirmDialog } from '../shared/ConfirmDialog'
import { Panel } from './Panel'
import { TwoColumn } from '../shared/TwoColumn'
import { toast } from '../../stores/toasts'

export function AddonsPanel({ agentId }: { agentId: string }) {
    const { data, error, reload, version } = useIntegrationList(agentId, 'addon')
    const [kinds, setKinds] = useState<Kind[]>()
    const [query, setQuery] = useState('')
    const [installing, setInstalling] = useState<Kind | null>(null)
    const [replacing, setReplacing] = useState<{ item: IntegrationItem; field: KindField } | null>(null)
    const [removing, setRemoving] = useState<IntegrationItem | null>(null)

    useEffect(() => {
        integrationKinds('addon').then(setKinds, () => setKinds([]))
    }, [])

    const q = query.trim().toLowerCase()
    const matches = (name: string, description: string) => !q || name.toLowerCase().includes(q) || description.toLowerCase().includes(q)
    const kindOf = (id: string) => kinds?.find((k) => k.id === id)
    const installed = (data?.items ?? []).filter((i) => matches(kindOf(i.kind)?.name ?? i.name, kindOf(i.kind)?.description ?? ''))
    const available = useMemo(
        () => (kinds ?? []).filter((k) => !data?.items.some((i) => i.kind === k.id) && matches(k.name, k.description)),
        [kinds, data, q]
    )

    const change = async (what: Promise<unknown>, failure: string) => {
        try {
            await what
            await reload()
        } catch (e) {
            toast.error(failure, e instanceof Error ? e.message : undefined)
        }
    }

    return (
        <Panel
            meta="Extra tools for this agent · each addon’s tools are named after it"
            actions={<SearchField aria-label="Search addons" placeholder="Search addons" value={query} onValueChange={setQuery} className="w-260" />}
        >
            <TwoColumn
                aside={
                    <>
                        <SelfManagementCard agentId={agentId} family="addon" onChanged={reload} />
                        <ActivityCard agentId={agentId} version={version} />
                    </>
                }
            >

            {error ? (
                <EmptyState kind="error" title="Couldn’t load addons" message={error.message} />
            ) : !data || !kinds ? (
                <Skeleton className="h-120" />
            ) : (
                <>
                    {installed.map((item) => {
                        const kind = kindOf(item.kind)
                        const state = cardState(item.status, item.enabled)
                        const detail = stateDetail(item.status)
                        const secrets = (kind?.fields ?? []).filter((f) => f.type === 'secret')
                        const flags = (kind?.fields ?? []).filter((f) => f.type === 'bool')
                        return (
                            <IntegrationCard
                                key={item.id}
                                kind={item.kind}
                                name={kind?.name ?? item.name}
                                handle={kind?.description}
                                state={state}
                                enabled={item.enabled}
                                onEnabledChange={(enabled) => void change(updateIntegration(item.id, { enabled }), 'Couldn’t change the addon')}
                                failure={state === 'failed' && detail ? { title: 'This addon is not working', message: detail } : undefined}
                                actions={
                                    item.can_edit && (
                                        <Button variant="destructive" icon="trash" onClick={() => setRemoving(item)}>
                                            {item.legacy_linked ? 'Unlink' : 'Remove'}
                                        </Button>
                                    )
                                }
                            >
                                {secrets.map((f) => (
                                    <SecretField key={f.key} label={f.label} masked={String(item.settings[f.key] ?? '')} onReplace={() => setReplacing({ item, field: f })} />
                                ))}
                                {flags.map((f) => (
                                    <SwitchRow
                                        key={f.key}
                                        label={f.label}
                                        checked={Boolean(item.settings[f.key] ?? f.default)}
                                        disabled={!item.can_edit}
                                        onCheckedChange={(v) => void change(updateIntegration(item.id, { settings: { ...item.settings, [f.key]: v } }), 'Couldn’t change the setting')}
                                    />
                                ))}
                                {ADDON_TOOLS[item.kind] && <ToolList tools={ADDON_TOOLS[item.kind]} />}
                            </IntegrationCard>
                        )
                    })}

                    {available.length > 0 && (
                        <div className="flex flex-col gap-12">
                            <p className="type-label uppercase text-faint">Available</p>
                            {available.map((k) => (
                                <Card key={k.id} title={k.name} description={k.description} headerTrailing={data.can_manage ? <Button onClick={() => setInstalling(k)}>Install</Button> : undefined}>
                                    <IntegrationLogo kind={k.id} size={28} />
                                </Card>
                            ))}
                        </div>
                    )}

                    {installed.length === 0 && available.length === 0 && <EmptyState kind="no-results" title="No addon matches" message="Try a different search." />}

                    {available.length === 0 && (
                        <Callout title="That’s every addon in this version.">New addons ship with Vizier releases. For anything else, connect an MCP server.</Callout>
                    )}
                </>
            )}

            </TwoColumn>

            <InstallDialog
                kind={installing}
                onClose={() => setInstalling(null)}
                onInstall={async (kind, settings) => {
                    await createIntegration(agentId, { kind: kind.id, name: kind.id, settings })
                    setInstalling(null)
                    await reload()
                }}
            />

            <Dialog open={replacing !== null} title={`Replace ${replacing?.field.label.toLowerCase() ?? 'secret'}`} confirmLabel="" onConfirm={() => {}} onCancel={() => setReplacing(null)}>
                {replacing && (
                    <ReplaceSecret
                        field={replacing.field}
                        onCancel={() => setReplacing(null)}
                        onSave={async (value) => {
                            await updateIntegration(replacing.item.id, { settings: { ...replacing.item.settings, [replacing.field.key]: value } })
                            setReplacing(null)
                            await reload()
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={removing !== null}
                title={`${removing?.legacy_linked ? 'Unlink' : 'Remove'} ${kindOf(removing?.kind ?? '')?.name ?? 'addon'}?`}
                message="The agent loses its tools from the next turn."
                confirmLabel={removing?.legacy_linked ? 'Unlink' : 'Remove addon'}
                onConfirm={() => {
                    const target = removing
                    setRemoving(null)
                    if (target) void change(removeIntegration(agentId, target.id), 'Couldn’t remove the addon')
                }}
                onCancel={() => setRemoving(null)}
            />
        </Panel>
    )
}

function ReplaceSecret({ field, onCancel, onSave }: { field: KindField; onCancel: () => void; onSave: (value: string) => Promise<void> }) {
    const [value, setValue] = useState('')
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()
    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                try {
                    await onSave(value.trim())
                } catch (err) {
                    setProblem(err instanceof Error ? err.message : 'It could not be saved.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            <TextField label={`New ${field.label.toLowerCase()}`} mono type="password" autoComplete="off" value={value} invalid={!!problem} helper={problem} onChange={(e) => setValue(e.target.value)} autoFocus />
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>Cancel</Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!value.trim()}>Save</Button>
            </div>
        </form>
    )
}

function InstallDialog({ kind, onClose, onInstall }: { kind: Kind | null; onClose: () => void; onInstall: (kind: Kind, settings: Record<string, unknown>) => Promise<void> }) {
    return (
        <Dialog open={kind !== null} title={`Install ${kind?.name ?? 'addon'}`} confirmLabel="" onConfirm={() => {}} onCancel={onClose}>
            {kind && <InstallForm key={kind.id} kind={kind} onClose={onClose} onInstall={onInstall} />}
        </Dialog>
    )
}

function InstallForm({ kind, onClose, onInstall }: { kind: Kind; onClose: () => void; onInstall: (kind: Kind, settings: Record<string, unknown>) => Promise<void> }) {
    const [values, setValues] = useState<Record<string, unknown>>(() => Object.fromEntries(kind.fields.filter((f) => f.default !== undefined).map((f) => [f.key, f.default])))
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()
    const ready = kind.fields.every((f) => !f.required || String(values[f.key] ?? '').trim() !== '')
    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setProblem(undefined)
                try {
                    await onInstall(kind, values)
                } catch (err) {
                    setProblem(err instanceof ApiError || err instanceof Error ? err.message : 'It could not be installed.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {kind.fields.map((f) =>
                f.type === 'bool' ? (
                    <SwitchRow key={f.key} label={f.label} checked={Boolean(values[f.key])} onCheckedChange={(v) => setValues((s) => ({ ...s, [f.key]: v }))} />
                ) : (
                    <TextField
                        key={f.key}
                        label={f.label}
                        mono={f.type === 'secret'}
                        type={f.type === 'secret' ? 'password' : 'text'}
                        autoComplete="off"
                        value={String(values[f.key] ?? '')}
                        onChange={(e) => setValues((s) => ({ ...s, [f.key]: e.target.value }))}
                    />
                )
            )}
            {problem && <p className="type-small text-failed-text">{problem}</p>}
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onClose}>Cancel</Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!ready}>Install</Button>
            </div>
        </form>
    )
}
