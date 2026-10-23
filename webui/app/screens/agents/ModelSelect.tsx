import { HighlightMatch, Icon, Picker, ProviderLogo, Select, Skeleton } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { chatProviders, providerModels, type ModelSummary, type ProviderSummary } from '../../api/endpoints/catalogue'
import { capabilityLine } from '../../lib/models'

/** Past this many models a Select becomes a searchable Picker. */
const SEARCH_ABOVE = 8

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
}

let providerCache: Promise<ProviderSummary[]> | undefined
const modelCache = new Map<string, Promise<ModelSummary[]>>()

const loadProviders = () => (providerCache ??= chatProviders().catch((e) => ((providerCache = undefined), Promise.reject(e))))
const loadModels = (id: string) => {
    let hit = modelCache.get(id)
    if (!hit) {
        hit = providerModels(id).catch((e) => (modelCache.delete(id), Promise.reject(e)))
        modelCache.set(id, hit)
    }
    return hit
}

/**
 * Provider and model for an agent: a Select with the provider's logo, then the provider's models
 * from the catalogue (searchable past a handful), with what the chosen model can do underneath.
 */
export function ModelSelect({ provider, model, onChange, providerLabel = 'Provider', modelLabel = 'Model', disabled, needsImages }: Props) {
    const [providers, setProviders] = useState<ProviderSummary[]>()
    const [models, setModels] = useState<ModelSummary[]>()
    const [failed, setFailed] = useState(false)
    const [query, setQuery] = useState('')

    useEffect(() => {
        let live = true
        loadProviders().then(
            (p) => live && setProviders(p),
            () => live && setFailed(true)
        )
        return () => {
            live = false
        }
    }, [])

    useEffect(() => {
        if (!provider) return
        let live = true
        setModels(undefined)
        loadModels(provider).then(
            (m) => live && setModels(m),
            () => live && setModels([])
        )
        return () => {
            live = false
        }
    }, [provider])

    const usable = useMemo(() => (models ?? []).filter((m) => !needsImages || m.caps.image_input), [models, needsImages])
    const current = usable.find((m) => m.id === model) ?? models?.find((m) => m.id === model)
    const providerName = providers?.find((p) => p.id === provider)?.name ?? provider

    const providerOptions = useMemo(() => {
        const list = providers ?? []
        // A provider saved on the agent stays choosable even when the catalogue no longer lists it.
        const all = provider && !list.some((p) => p.id === provider) ? [...list, { id: provider, name: provider } as ProviderSummary] : list
        return all.map((p) => ({
            value: p.id,
            label: p.name,
            leading: <ProviderLogo provider={p.id} name={p.name} size="sm" />,
        }))
    }, [providers, provider])

    const pickProvider = async (id: string) => {
        if (id === provider) return
        const first = (await loadModels(id).catch(() => [])).find((m) => (!needsImages || m.caps.image_input) && m.status !== 'deprecated')
        onChange({ provider: id, model: first?.id ?? '', info: first })
    }

    const filtered = useMemo(() => {
        const q = query.trim().toLowerCase()
        return q ? usable.filter((m) => m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q)) : usable
    }, [usable, query])

    if (failed) {
        return <p className="type-small text-failed-text">The model catalogue could not be loaded.</p>
    }

    return (
        <div className="flex flex-col gap-8">
            <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
                {providers ? (
                    <Select label={providerLabel} options={providerOptions} value={provider} onChange={pickProvider} disabled={disabled} />
                ) : (
                    <Skeleton className="h-40" />
                )}
                {!provider ? (
                    <Select label={modelLabel} options={[]} onChange={() => {}} placeholder="Choose a provider first" disabled />
                ) : !models ? (
                    <Skeleton className="h-40" />
                ) : usable.length > SEARCH_ABOVE ? (
                    <div className="flex flex-col gap-6">
                        <span className="type-ui-medium text-soft">{modelLabel}</span>
                        <Picker<ModelSummary>
                            label={`${providerName} models`}
                            width={420}
                            itemHeight={52}
                            trigger={
                                <button
                                    type="button"
                                    disabled={disabled}
                                    aria-label={`${modelLabel}: ${model || 'none chosen'}`}
                                    className="flex h-40 w-full items-center justify-between gap-8 rounded-md border border-border-strong bg-ground pr-10 pl-12 text-left type-code text-fg hover:border-border-hover focus-visible:border-live focus-visible:outline-none disabled:opacity-40"
                                >
                                    <span className={model ? 'truncate' : 'truncate text-faint'}>{model || 'Choose…'}</span>
                                    <Icon name="chevron-down" size={14} tone="muted" />
                                </button>
                            }
                            search={{ placeholder: 'Search models', total: `${usable.length}`, value: query, onChange: setQuery }}
                            sections={[{ items: filtered }]}
                            getKey={(m) => m.id}
                            renderItem={(m, { query: q }) => (
                                <div className="flex min-w-0 flex-col">
                                    <span className="truncate type-code text-fg">
                                        <HighlightMatch text={m.id} query={q} />
                                    </span>
                                    <span className="truncate type-small text-faint">{capabilityLine(m)}</span>
                                </div>
                            )}
                            onSelect={(m) => onChange({ provider, model: m.id, info: m })}
                        />
                    </div>
                ) : (
                    <Select
                        label={modelLabel}
                        mono
                        options={usable.map((m) => ({ value: m.id, label: m.id }))}
                        value={model}
                        onChange={(id) => onChange({ provider, model: id, info: usable.find((m) => m.id === id) })}
                        placeholder={usable.length ? 'Choose…' : 'No models listed'}
                        disabled={disabled || usable.length === 0}
                    />
                )}
            </div>
            {current && <p className="type-small text-faint">{capabilityLine(current)}</p>}
            {models && !current && model && <p className="type-small text-faint">Not in the catalogue. It is used as typed.</p>}
        </div>
    )
}
