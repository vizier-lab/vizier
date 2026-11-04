import { HighlightMatch, Icon, Picker, ProviderLogo, type PickerSection } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { type ModelSummary, type ProviderSummary } from '../../api/endpoints/catalogue'
import { loadModels, loadProviders, logoOf } from '../../lib/catalogue'
import { useSession } from '../../stores/session'
import type { ModelOverride } from '../../api/frames'

const MEDIA_ONLY = new Set(['elevenlabs', 'voyageai', 'hyperbolic'])
type Choice = { provider: string; model: string; name: string; providerInfo?: ProviderSummary; modelInfo?: ModelSummary; isDefault?: boolean; unavailable?: boolean }

type Props = {
    value: ModelOverride | null
    /** The agent's own model, shown when no override is chosen. */
    defaultModel?: { provider: string; model: string } | null
    onChange: (choice: ModelOverride | null) => void
    onAvailabilityChange?: (available: boolean) => void
}

const triggerClass = 'flex h-28 w-[240px] min-w-0 max-w-full items-center justify-end gap-6 rounded-md border border-transparent bg-transparent pr-6 pl-8 text-right type-mono-xs text-muted hover:text-fg focus-visible:border-live focus-visible:outline-none'
const label = (choice: Pick<Choice, 'provider' | 'model'>) => `${choice.provider}/${choice.model}`

/** One searchable provider/model choice. The provider and complete model id remain separate. */
export function ModelOverridePicker({ value, defaultModel, onChange, onAvailabilityChange }: Props) {
    const [query, setQuery] = useState('')
    const [providers, setProviders] = useState<ProviderSummary[]>()
    const [choices, setChoices] = useState<Choice[]>([])
    const [loading, setLoading] = useState(true)
    const [failed, setFailed] = useState(false)
    const userId = useSession((state) => state.user?.user_id)

    useEffect(() => {
        let live = true
        setLoading(true)
        void loadProviders().then(async (all) => {
            const eligible = all.filter((provider) =>
                provider.dialect != null && provider.kind !== 'media' && !MEDIA_ONLY.has(provider.id) &&
                (provider.connection.status === 'connected' || provider.kind === 'local') &&
                (provider.id !== 'dummyplug' || provider.origin === 'builtin') &&
                (provider.id !== 'dummyplug' || !!userId)
            )
            const lists = await Promise.all(eligible.map(async (provider) => ({
                provider,
                models: await loadModels(provider.id).catch(() => null),
            })))
            if (!live) return
            setProviders(eligible)
            if (lists.some((row) => row.models === null)) throw new Error('model catalogue request failed')
            setChoices(lists.flatMap(({ provider, models }) => (models ?? []).map((model) => ({
                provider: provider.id,
                model: model.id,
                name: model.name,
                providerInfo: provider,
                modelInfo: model,
            }))))
            setFailed(false)
        }).catch(() => {
            if (live) setFailed(true)
        }).finally(() => {
            if (live) setLoading(false)
        })
        return () => { live = false }
    }, [userId])

    const loaded = !loading && !failed
    const saved = value && choices.find((choice) => choice.provider === value.provider && choice.model === value.model)
    const stale: Choice | null = value && loaded && !saved
        ? { provider: value.provider, model: value.model, name: label(value), unavailable: true }
        : null
    const selected = value ? (saved ?? stale) : null
    const options = useMemo(() => {
        const q = query.trim().toLowerCase()
        const matching = choices.filter((choice) => !q || label(choice).toLowerCase().includes(q) || choice.name.toLowerCase().includes(q))
        return [{ provider: '', model: '', name: 'Agent default', isDefault: true }, ...matching]
    }, [choices, query])
    const sections: PickerSection<Choice>[] = [{ items: options }]
    // A saved choice missing from the list stays on the trigger, but as a refused row it is not
    // offered again as if it could be picked.
    const selectedProvider = providers?.find((provider) => provider.id === value?.provider)
    const shownProvider = value ? selectedProvider : providers?.find((provider) => provider.id === defaultModel?.provider)
    const available = value === null || !loaded || (!!saved && !failed)
    const notifyAvailability = useMemo(() => onAvailabilityChange, [onAvailabilityChange])

    useEffect(() => {
        notifyAvailability?.(available)
    }, [available, notifyAvailability])

    // A saved choice is named only once the catalogue has confirmed it; until then (and for an
    // unverified value) the selector reads Agent default rather than naming a model it may not
    // be able to use.
    const agentModel = defaultModel ? label(defaultModel) : null
    const shown = value && loaded && saved ? label(value) : null
    const triggerText = shown ?? agentModel ?? 'Agent default'
    return (
        <div className="ml-auto flex w-[240px] min-w-0 shrink flex-col items-end gap-4 [&>div]:min-w-0 [&>div]:max-w-full">
            <Picker<Choice>
                label="Chat model"
                width={480}
                align="end"
                itemHeight={48}
                trigger={
                    <button type="button" aria-label={`Chat model: ${shown ?? 'Agent default'}`} className={triggerClass}>
                        {shownProvider && <ProviderLogo provider={shownProvider.id} name={shownProvider.name} size="sm" {...logoOf(shownProvider)} />}
                        <span className="min-w-0 truncate">
                            {triggerText}
                        </span>
                        {stale && <span className="shrink-0 type-small text-retrying">unavailable</span>}
                        <Icon name="chevron-down" size={12} tone="faint" />
                    </button>
                }
                search={{ placeholder: 'Search provider and model', value: query, onChange: setQuery, total: loading ? undefined : `${choices.length}` }}
                sections={sections}
                getKey={(choice) => choice.isDefault ? 'agent-default' : `${choice.provider}/${choice.model}`}
                loading={loading}
                empty={failed ? 'The model catalogue could not be loaded.' : choices.length === 0 ? 'No eligible chat models are available.' : 'No matching models.'}
                onSelect={(choice) => {
                    if (choice.unavailable) return
                    setQuery('')
                    onChange(choice.isDefault ? null : { provider: choice.provider, model: choice.model })
                }}
                renderItem={(choice, { query: search, active }) => choice.isDefault ? (
                    <span className="type-ui text-fg">Agent default</span>
                ) : (
                    <div className={`flex min-w-0 flex-1 items-center gap-10 ${choice.unavailable && !active ? 'opacity-50' : ''}`}>
                        <ProviderLogo provider={choice.provider} name={choice.providerInfo?.name ?? choice.provider} size="sm" {...logoOf(choice.providerInfo)} />
                        <div className="flex min-w-0 flex-1 flex-col">
                            <span className="truncate type-ui text-fg"><HighlightMatch text={label(choice)} query={search} /></span>
                            <span className="truncate type-small text-faint">{choice.name}</span>
                        </div>
                        {choice.unavailable && <span className="shrink-0 type-small text-retrying">unavailable</span>}
                    </div>
                )}
            />
            {failed ? <span className="type-small text-failed-text">Couldn’t load chat models.</span>
                : choices.length === 0 && !loading ? <span className="type-small text-faint">No eligible chat models are available.</span>
                    : null}
            {stale && (
                <span className="type-small text-retrying">This saved model is unavailable. Choose another model or Agent default.</span>
            )}
        </div>
    )
}
