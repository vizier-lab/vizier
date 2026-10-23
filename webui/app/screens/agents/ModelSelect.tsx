import { Badge, Button, Checkbox, HighlightMatch, Icon, KeyValueList, Picker, ProviderLogo, Skeleton, type PickerSection } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { type ModelSummary, type ProviderSummary } from '../../api/endpoints/catalogue'
import { PERMISSIONS, useCan } from '../../lib/access'
import { loadModels, logoOf, useCatalogueProviders } from '../../lib/catalogue'
import { abbreviate } from '../../lib/format'
import { capabilityLine } from '../../lib/models'
import { ConnectForm } from '../providers/ConnectForm'
import { toast } from '../../stores/toasts'

type Props = {
    provider: string
    model: string
    onChange: (choice: { provider: string; model: string; info?: ModelSummary }) => void
    /** Field labels, so one screen can hold a second pair ("Dream provider"). */
    providerLabel?: string
    modelLabel?: string
    disabled?: boolean
    /** Models must be able to read images (the vision model). */
    needsImages?: boolean
    /** Leave out the card of limits and prices under the pair (secondary models). */
    compact?: boolean
}

const trigger =
    'flex h-40 w-full items-center gap-8 rounded-md border border-border-strong bg-ground pr-10 pl-12 text-left type-body text-fg hover:border-border-hover focus-visible:border-live focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40'

/** "$3 in / $15 out", for the model rows. */
const price = (m: ModelSummary) => (m.cost ? `$${+m.cost.input.toFixed(2)} / $${+m.cost.output.toFixed(2)}` : '')

/** Tools, vision and reasoning as the short words a row has room for. */
const caps = (m: ModelSummary) =>
    [m.caps.tool_call && 'tools', m.caps.image_input && 'vision', m.caps.reasoning && 'reasoning'].filter(Boolean).join(' · ')

/** A catalogue local provider counts as connected without any setup; it is listed as saved once something was. */
const isConnected = (p: ProviderSummary) => p.connection.status === 'connected' && (p.connection.source != null || p.origin === 'builtin')

/**
 * Provider and model for an agent, in one place. The provider list is the catalogue (connected
 * ones first, then custom, then every other), the model list is searchable and takes a typed id,
 * and a provider that isn't connected yet gets its credentials form right here, so choosing a
 * model never sends anyone to Settings and back.
 */
export function ModelSelect({ provider, model, onChange, providerLabel = 'Provider', modelLabel = 'Model', disabled, needsImages, compact }: Props) {
    const { providers, failed, reload } = useCatalogueProviders()
    const canConnect = useCan(PERMISSIONS.providers)
    const [models, setModels] = useState<ModelSummary[]>()
    const [showAll, setShowAll] = useState(false)
    const [providerQuery, setProviderQuery] = useState('')
    const [modelQuery, setModelQuery] = useState('')
    // The provider whose connect form was closed without connecting.
    const [dismissed, setDismissed] = useState<string | null>(null)

    useEffect(() => {
        if (!provider) return setModels(undefined)
        let live = true
        setModels(undefined)
        loadModels(provider, showAll).then(
            (m) => live && setModels(m),
            () => live && setModels([])
        )
        return () => {
            live = false
        }
    }, [provider, showAll])

    const selected = providers?.find((p) => p.id === provider)
    const info = models?.find((m) => m.id === model)
    const usable = useMemo(() => (models ?? []).filter((m) => !(needsImages && m.caps.image_input === false)), [models, needsImages])

    const providerSections = useMemo<PickerSection<ProviderSummary>[]>(() => {
        // dummyplug is a development provider: it is only listed while an agent already runs on it.
        const list = (providers ?? []).filter((p) => p.kind !== 'media' && (p.id !== 'dummyplug' || p.id === provider))
        const q = providerQuery.trim().toLowerCase()
        const match = list.filter((p) => !q || p.name.toLowerCase().includes(q) || p.id.includes(q))
        return [
            { label: 'Connected', items: match.filter((p) => p.origin !== 'custom' && isConnected(p)) },
            { label: 'Custom', items: match.filter((p) => p.origin === 'custom') },
            { label: 'All providers', items: match.filter((p) => p.origin !== 'custom' && !isConnected(p)) },
        ]
    }, [providers, providerQuery, provider])

    const modelSections = useMemo<PickerSection<ModelSummary>[]>(() => {
        const q = modelQuery.trim()
        const lower = q.toLowerCase()
        const match = usable.filter((m) => !lower || m.id.toLowerCase().includes(lower) || m.name.toLowerCase().includes(lower))
        const typed: ModelSummary[] =
            q && !usable.some((m) => m.id === q)
                ? [{ id: q, name: `Use “${q}”`, default: false, limit: {}, caps: {}, status: 'active', origin: 'typed' }]
                : []
        return [{ items: typed }, { items: match }]
    }, [usable, modelQuery])

    const pickProvider = async (p: ProviderSummary) => {
        setProviderQuery('')
        setDismissed(null)
        if (p.id === provider) return
        const all = await loadModels(p.id).catch(() => [] as ModelSummary[])
        const ok = all.filter((m) => !(needsImages && m.caps.image_input === false) && m.status !== 'deprecated')
        const first = ok.find((m) => m.default) ?? ok[0]
        onChange({ provider: p.id, model: first?.id ?? '', info: first })
    }

    if (failed) return <p className="type-small text-failed-text">The model catalogue could not be loaded.</p>

    const connecting = selected && selected.connection.status === 'missing' && selected.origin !== 'custom' && dismissed !== selected.id ? selected : undefined

    return (
        <div className="flex flex-col gap-12">
            <div className="grid grid-cols-1 gap-16 sm:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
                <div className="flex flex-col gap-6">
                    <span className="type-ui-medium text-soft">{providerLabel}</span>
                    {providers ? (
                        <Picker<ProviderSummary>
                            label={`${providerLabel}s`}
                            width={360}
                            itemHeight={44}
                            trigger={
                                <button type="button" disabled={disabled} aria-label={`${providerLabel}: ${selected?.name ?? (provider || 'none chosen')}`} className={trigger}>
                                    {provider ? <ProviderLogo provider={provider} name={selected?.name ?? provider} size="sm" {...logoOf(selected)} /> : null}
                                    <span className={provider ? 'min-w-0 flex-1 truncate' : 'min-w-0 flex-1 truncate text-faint'}>{provider ? (selected?.name ?? provider) : 'Choose a provider'}</span>
                                    {selected?.connection.status === 'missing' && selected.origin !== 'custom' && <span className="type-small text-retrying">not connected</span>}
                                    <Icon name="chevron-down" size={14} tone="muted" />
                                </button>
                            }
                            search={{ placeholder: 'Search providers', total: `${providers.filter((p) => p.kind !== 'media').length}`, value: providerQuery, onChange: setProviderQuery }}
                            sections={providerSections}
                            getKey={(p) => p.id}
                            renderItem={(p, { query }) => (
                                <div className="flex min-w-0 flex-1 items-center gap-10">
                                    <ProviderLogo provider={p.id} name={p.name} size="sm" {...logoOf(p)} />
                                    <span className="min-w-0 flex-1 truncate type-ui text-fg">
                                        <HighlightMatch text={p.name} query={query} />
                                    </span>
                                    {p.origin === 'custom' && <Badge>custom</Badge>}
                                    <span className={isConnected(p) ? 'type-mono-xs text-live' : 'type-mono-xs text-faint'}>
                                        {isConnected(p) ? (p.connection.source?.startsWith('env:') ? 'env' : 'connected') : ''}
                                    </span>
                                </div>
                            )}
                            onSelect={(p) => void pickProvider(p)}
                            remainder={providerSections.every((s) => s.items.length === 0) ? 'No provider matched. Add a custom one in Settings › Providers.' : undefined}
                        />
                    ) : (
                        <Skeleton className="h-40" />
                    )}
                </div>

                <div className="flex flex-col gap-6">
                    <span className="type-ui-medium text-soft">{modelLabel}</span>
                    {!provider ? (
                        <button type="button" disabled className={trigger}>
                            <span className="flex-1 text-faint">Choose a provider first</span>
                            <Icon name="chevron-down" size={14} tone="muted" />
                        </button>
                    ) : (
                        <Picker<ModelSummary>
                            label={`${selected?.name ?? provider} models`}
                            width={520}
                            itemHeight={52}
                            trigger={
                                <button type="button" disabled={disabled} aria-label={`${modelLabel}: ${model || 'none chosen'}`} className={trigger}>
                                    {info && <ProviderLogo provider={provider} name={info.name} size="sm" {...logoOf(info)} />}
                                    <span className={model ? 'flex min-w-0 flex-1 items-baseline gap-8' : 'min-w-0 flex-1 truncate text-faint'}>
                                        {model ? (
                                            <>
                                                <span className="truncate">{info?.name ?? model}</span>
                                                {info && info.name !== model && <span className="truncate type-mono-xs text-faint">{model}</span>}
                                            </>
                                        ) : (
                                            'Choose a model'
                                        )}
                                    </span>
                                    {models && model && !info && <span className="type-small text-faint">not listed</span>}
                                    <Icon name="chevron-down" size={14} tone="muted" />
                                </button>
                            }
                            search={{ placeholder: 'Search, or type a model id', total: models ? `${usable.length}` : undefined, value: modelQuery, onChange: setModelQuery }}
                            sections={modelSections}
                            getKey={(m) => m.id}
                            loading={!models}
                            renderItem={(m, { query }) =>
                                m.origin === 'typed' ? (
                                    <span className="truncate type-ui text-fg">
                                        Use <span className="type-code">{m.id}</span> <span className="text-faint">· not in the list</span>
                                    </span>
                                ) : (
                                    <div className="flex min-w-0 flex-1 items-center gap-10">
                                        <ProviderLogo provider={provider} name={m.name} size="sm" {...logoOf(m)} />
                                        <div className="flex min-w-0 flex-1 flex-col">
                                            <span className="truncate type-ui text-fg">
                                                <HighlightMatch text={m.name} query={query} />
                                                {m.status !== 'active' && <span className="ml-6 type-mono-xs text-retrying">{m.status}</span>}
                                                {m.origin === 'server' && <span className="ml-6 type-mono-xs text-faint">from server</span>}
                                            </span>
                                            <span className="truncate type-mono-xs text-faint">{m.id}</span>
                                        </div>
                                        <div className="flex shrink-0 flex-col items-end">
                                            <span className="type-mono-xs text-muted">{m.limit.context ? `${abbreviate(m.limit.context)} ctx` : ''}</span>
                                            <span className="type-mono-xs text-faint">{price(m) || caps(m)}</span>
                                        </div>
                                    </div>
                                )
                            }
                            onSelect={(m) => {
                                setModelQuery('')
                                onChange({ provider, model: m.id, info: m.origin === 'typed' ? undefined : m })
                            }}
                            remainder={models && usable.length === 0 ? 'No models listed. Type a model id above.' : undefined}
                            footer={<Checkbox label="Show all models, including deprecated and ones without tool calling" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} />}
                        />
                    )}
                </div>
            </div>

            {connecting && (
                <div className="flex flex-col gap-12 rounded-lg border border-dashed border-border-strong p-16">
                    <div className="flex flex-col gap-2">
                        <span className="type-ui-medium text-fg">Connect {connecting.name} to use it</span>
                        <span className="type-small text-faint">
                            {canConnect ? 'Its credentials are saved in Vizier and shared with every agent that uses this provider.' : `${connecting.name} isn’t connected yet. Ask someone who manages providers to connect it.`}
                        </span>
                    </div>
                    {canConnect && (
                        <ConnectForm
                            key={connecting.id}
                            provider={connecting}
                            onCancel={() => setDismissed(connecting.id)}
                            onDone={() => {
                                toast.success(`${connecting.name} connected`)
                                void reload()
                            }}
                        />
                    )}
                    {!canConnect && (
                        <div>
                            <Button onClick={() => setDismissed(connecting.id)}>Dismiss</Button>
                        </div>
                    )}
                </div>
            )}

            {info && !compact && <ModelCard model={info} />}
            {info && compact && <p className="type-small text-faint">{capabilityLine(info)}</p>}
            {models && !info && model && <p className="type-small text-faint">Not in the catalogue. It is used as typed.</p>}
        </div>
    )
}

const yesNo = (v?: boolean | null) => (v == null ? 'unknown' : v ? 'yes' : 'no')

/** The chosen model's limits, capabilities and price, from the catalogue. */
function ModelCard({ model: m }: { model: ModelSummary }) {
    const context = m.limit.input ?? m.limit.context
    const dollars = (n: number) => `$${+n.toFixed(n < 1 ? 3 : 2)}`
    const left = [
        { label: 'Context window', value: context ? `${context.toLocaleString()} tokens` : 'unknown' },
        { label: 'Max output', value: m.limit.output ? `${m.limit.output.toLocaleString()} tokens` : 'unknown' },
        { label: 'Price per 1M in / out', value: m.cost ? `${dollars(m.cost.input)} / ${dollars(m.cost.output)}` : 'unknown' },
    ]
    const right = [
        { label: 'Tool calling', value: yesNo(m.caps.tool_call) },
        { label: 'Image input', value: yesNo(m.caps.image_input) },
        { label: 'Reasoning', value: yesNo(m.caps.reasoning) },
    ]
    if (m.knowledge) right.push({ label: 'Knowledge', value: m.knowledge })
    return (
        <div className="grid grid-cols-1 gap-x-32 gap-y-8 rounded-lg border border-border bg-ground px-16 py-12 sm:grid-cols-2">
            <KeyValueList items={left} />
            <KeyValueList items={right} />
        </div>
    )
}
