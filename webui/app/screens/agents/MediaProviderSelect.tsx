import { HighlightMatch, Icon, Picker, ProviderLogo, Skeleton } from '@vizier/design'
import { useMemo, useState, type ReactNode } from 'react'
import { logoOf, useCatalogueProviders } from '../../lib/catalogue'
import { mediaProviderRows, mediaProviderSections, needsConnecting, type MediaProviderRow } from '../../lib/mediaProviders'
import { ProviderConnectPanel } from './ProviderConnectPanel'

type Props = {
    /** The setting values this capability accepts, in the server's order. */
    values: string[]
    value: string
    onChange: (value: string) => void
    label?: string
    /** The model, voice and other fields drawn beside the provider. */
    children?: ReactNode
    /** Grid classes for the provider and its fields. */
    grid?: string
}

const trigger =
    'flex h-40 w-full items-center gap-8 rounded-md border border-border-strong bg-ground pr-10 pl-12 text-left type-body text-fg hover:border-border-hover focus-visible:border-live focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40'

const status = (r: MediaProviderRow) =>
    r.local ? 'local' : r.connected ? (r.entry?.connection.source?.startsWith('env:') ? 'env' : 'connected') : ''

/**
 * A media capability's provider, drawn as the language model's picker is: logos, what is connected,
 * and the credentials form right below when the chosen provider isn't connected yet.
 */
export function MediaProviderSelect({ values, value, onChange, label = 'Provider', children, grid = 'grid grid-cols-1 gap-16 sm:grid-cols-2' }: Props) {
    // A catalogue that can't load still lists the providers, only without logos or connection.
    const { providers, failed, reload } = useCatalogueProviders()
    const [query, setQuery] = useState('')
    const [dismissed, setDismissed] = useState<string | null>(null)
    const rows = useMemo(() => mediaProviderRows(values, providers), [values, providers])
    const sections = useMemo(() => mediaProviderSections(rows, query), [rows, query])
    const selected = rows.find((r) => r.value === value)
    const connecting = needsConnecting(selected)
    const shownConnect = connecting && dismissed !== connecting.id ? connecting : undefined

    return (
        <div className="flex flex-col gap-12">
            <div className={grid}>
                <div className="flex flex-col gap-6">
                    <span className="type-ui-medium text-soft">{label}</span>
                    {providers || failed ? (
                        <Picker<MediaProviderRow>
                            label={`${label}s`}
                            width={320}
                            itemHeight={44}
                            trigger={
                                <button type="button" aria-label={`${label}: ${selected?.name ?? (value || 'none chosen')}`} className={trigger}>
                                    {selected && <ProviderLogo provider={selected.entry?.id ?? selected.value} name={selected.name} size="sm" {...logoOf(selected.entry)} />}
                                    <span className={selected ? 'min-w-0 flex-1 truncate' : 'min-w-0 flex-1 truncate text-faint'}>{selected?.name ?? (value || 'Choose a provider')}</span>
                                    {connecting && <span className="type-small text-retrying">not connected</span>}
                                    <Icon name="chevron-down" size={14} tone="muted" />
                                </button>
                            }
                            search={{ placeholder: 'Search providers', total: `${rows.length}`, value: query, onChange: setQuery }}
                            sections={sections}
                            getKey={(r) => r.value}
                            renderItem={(r, { query: q }) => (
                                <div className="flex min-w-0 flex-1 items-center gap-10">
                                    <ProviderLogo provider={r.entry?.id ?? r.value} name={r.name} size="sm" {...logoOf(r.entry)} />
                                    <span className="min-w-0 flex-1 truncate type-ui text-fg">
                                        <HighlightMatch text={r.name} query={q} />
                                    </span>
                                    <span className={r.connected ? 'type-mono-xs text-live-text' : 'type-mono-xs text-faint'}>{status(r)}</span>
                                </div>
                            )}
                            onSelect={(r) => {
                                setQuery('')
                                setDismissed(null)
                                if (r.value !== value) onChange(r.value)
                            }}
                            remainder={sections.every((s) => s.items.length === 0) ? 'No provider matched.' : undefined}
                        />
                    ) : (
                        <Skeleton className="h-40" />
                    )}
                </div>
                {children}
            </div>
            {shownConnect && <ProviderConnectPanel provider={shownConnect} onDismiss={() => setDismissed(shownConnect.id)} onConnected={() => void reload()} />}
        </div>
    )
}
