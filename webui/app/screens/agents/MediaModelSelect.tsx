import { Badge, Button, HighlightMatch, Icon, Picker } from '@vizier/design'
import { useState } from 'react'
import type { MediaCapability } from '../../api/endpoints/catalogue'
import { PERMISSIONS, useCan } from '../../lib/access'
import { modelSections, sourceLine, triggerLabel, type ModelRow } from '../../lib/mediaModels'
import type { MediaModelsState } from './useMediaModels'

type Props = {
    capability: MediaCapability
    label?: string
    /** The stored model id; empty means the documented default. */
    model: string
    /** The local defaults map's value, shown before the list has loaded. */
    fallbackDefault: string
    media: MediaModelsState
    onChange: (model: string) => void
}

const trigger =
    'flex h-40 w-full items-center gap-8 rounded-md border border-border-strong bg-ground pr-10 pl-12 text-left type-body text-fg hover:border-border-hover focus-visible:border-live focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40'

/**
 * A searchable OpenRouter media model picker (spec 028, FR-016/FR-021): the documented default
 * first, the capability's models with readable names and ids, and a typed-id row for anything
 * missing. Opening it is what fetches the list; a refresh only replaces the options and never
 * touches the chosen value.
 */
export function MediaModelSelect({ capability, label = 'Model', model, fallbackDefault, media, onChange }: Props) {
    const canRefresh = useCan(PERMISSIONS.providers)
    const [query, setQuery] = useState('')
    const sections = modelSections(media.data?.models, query)
    const shown = triggerLabel(media.data?.models, model, fallbackDefault)
    const refreshOff = media.data !== undefined && media.data.refresh.enabled === false

    return (
        <div className="flex flex-col gap-6">
            <span className="type-ui-medium text-soft">{label}</span>
            <Picker<ModelRow>
                label={label}
                width={520}
                itemHeight={52}
                onOpenChange={(open) => {
                    if (open) media.ensure()
                }}
                trigger={
                    <button type="button" aria-label={`${label}: ${model || shown.name || 'none chosen'}`} className={trigger}>
                        <span className={model || shown.name ? 'flex min-w-0 flex-1 items-baseline gap-8' : 'min-w-0 flex-1 truncate text-faint'}>
                            {model || shown.name ? (
                                <>
                                    {!model && <span className="shrink-0 type-mono-xs text-faint">Default ·</span>}
                                    <span className="truncate">{shown.name}</span>
                                    {shown.id && <span className="truncate type-mono-xs text-faint">{shown.id}</span>}
                                </>
                            ) : (
                                'Choose a model'
                            )}
                        </span>
                        {model && !media.data?.models.some((m) => m.id === model) && !media.loading && media.data && <span className="type-small text-faint">not listed</span>}
                        <Icon name="chevron-down" size={14} tone="muted" />
                    </button>
                }
                search={{ placeholder: 'Search, or type a model id', total: media.data ? `${media.data.models.length}` : undefined, value: query, onChange: setQuery }}
                sections={sections}
                getKey={(m) => m.id}
                loading={media.loading && !media.data}
                renderItem={(m, { query: q }) =>
                    m.typed ? (
                        <span className="truncate type-ui text-fg">
                            Use <span className="type-code">{m.id}</span> <span className="text-faint">· not in the list</span>
                        </span>
                    ) : (
                        <div className="flex min-w-0 flex-1 items-center gap-10">
                            <div className="flex min-w-0 flex-1 flex-col">
                                <span className="flex min-w-0 items-center gap-6 type-ui text-fg">
                                    <HighlightMatch text={m.name} query={q} />
                                    {m.default && <Badge>default</Badge>}
                                    {!m.listed && <span className="type-mono-xs text-retrying">not listed</span>}
                                </span>
                                <span className="truncate type-mono-xs text-faint">{m.id}</span>
                            </div>
                            {m.voices.length > 0 && <span className="shrink-0 type-mono-xs text-muted">{m.voices.length} voices</span>}
                            {(m.aspect_ratios.length > 0 || m.resolutions.length > 0) && (
                                <span className="shrink-0 type-mono-xs text-faint">{[...m.resolutions, ...m.aspect_ratios].slice(0, 3).join(' · ')}</span>
                            )}
                        </div>
                    )
                }
                onSelect={(m) => {
                    setQuery('')
                    onChange(m.id)
                }}
                remainder={media.data && media.data.models.length === 0 ? 'No models listed. Type a model id above.' : undefined}
            />
            <div className="flex items-center justify-between gap-8">
                <span className="type-small text-faint">{sourceLine(media.data) ?? ''}</span>
                {canRefresh && (
                    <Button
                        size="sm"
                        variant="ghost"
                        icon="refresh"
                        loading={media.loading}
                        disabled={refreshOff}
                        onClick={() => void media.refresh()}
                    >
                        {refreshOff ? 'Refresh is off' : 'Refresh'}
                    </Button>
                )}
            </div>
            {media.error && <p className="type-small text-failed-text">{media.error}</p>}
            {media.data?.refresh.last_error && !media.error && (
                <p className="type-small text-retrying">The last refresh failed: {media.data.refresh.last_error}. The last list is still usable.</p>
            )}
        </div>
    )
}
