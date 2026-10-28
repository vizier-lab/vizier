import { Button, FilterTabs, Icon, ProviderLogo, SearchField } from '@vizier/design'
import { useMemo, useState } from 'react'
import type { ProviderSummary } from '../../api/endpoints/catalogue'
import { logoOf } from '../../lib/catalogue'
import { count } from '../../lib/format'
import { ConnectForm } from './ConnectForm'

type Filter = 'all' | 'chat' | 'media' | 'local'

/** Providers whose credentials speech, image and embedding features use. */
const MEDIA = new Set(['openai', 'google', 'togetherai', 'github-copilot', 'groq', 'mistral', 'cohere', 'openrouter', 'huggingface', 'xai', 'elevenlabs', 'voyageai', 'hyperbolic', 'ollama', 'amazon-bedrock'])

const FILTERS: { value: Filter; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'chat', label: 'Chat' },
    { value: 'media', label: 'Media & embeddings' },
    { value: 'local', label: 'Local' },
]

type Props = {
    /** Providers not connected yet. */
    providers: ProviderSummary[]
    onConnected: (provider: ProviderSummary) => void
    /** Open the custom provider form, with a name when the search found nothing. */
    onCustom: (name?: string) => void
    /** On its own page: the list takes the height left, and "Add custom provider" is the page's. */
    page?: boolean
}

const sub = (p: ProviderSummary) => (p.model_count ? `${count(p.model_count)} models` : p.kind === 'local' ? 'local server' : p.kind === 'media' ? 'speech, images, embeddings' : '')

/**
 * Search the catalogue for a provider to connect. One list, popular first and then A to Z, and a
 * row opens its credentials form where it stands: nothing to dismiss, nowhere else to go.
 */
export function AddProvider({ providers, onConnected, onCustom, page }: Props) {
    const [query, setQuery] = useState('')
    const [filter, setFilter] = useState<Filter>('all')
    const [open, setOpen] = useState<string | null>(null)
    const q = query.trim().toLowerCase()

    const shown = useMemo(
        () =>
            providers
                .filter((p) => p.origin !== 'custom' && p.id !== 'dummyplug')
                .filter((p) => (filter === 'chat' ? p.kind !== 'media' && p.kind !== 'local' : filter === 'media' ? p.kind === 'media' || MEDIA.has(p.id) : filter === 'local' ? p.kind === 'local' : true))
                .filter((p) => !q || p.name.toLowerCase().includes(q) || p.id.includes(q))
                .sort((a, b) => Number(b.popular) - Number(a.popular) || a.name.localeCompare(b.name)),
        [providers, filter, q]
    )
    const popular = q ? 0 : shown.filter((p) => p.popular).length

    return (
        <div className={page ? 'flex min-h-0 flex-1 flex-col gap-16' : 'flex flex-col gap-12'}>
            <div className="flex flex-wrap items-center gap-12">
                <div className="min-w-220 flex-1">
                    <SearchField aria-label="Search providers" placeholder={`Search ${count(providers.length)} providers`} value={query} onValueChange={setQuery} />
                </div>
                <FilterTabs aria-label="Kind of provider" value={filter} onChange={setFilter} options={FILTERS} />
            </div>

            <div className={`overflow-y-auto rounded-lg border border-border bg-ground ${page ? 'min-h-0 flex-1' : 'max-h-[26rem]'}`} role="list" aria-label="Providers to connect">
                {shown.map((p, i) => (
                    <div key={p.id} role="listitem">
                        {popular > 0 && i === 0 && <p className="px-14 pt-10 pb-4 type-label uppercase text-faint">Popular</p>}
                        {popular > 0 && i === popular && <p className="border-t border-border px-14 pt-10 pb-4 type-label uppercase text-faint">A to Z</p>}
                        <button
                            type="button"
                            aria-expanded={open === p.id}
                            onClick={() => setOpen(open === p.id ? null : p.id)}
                            className="flex w-full items-center gap-12 px-14 py-8 text-left hover:bg-raised focus-visible:bg-raised focus-visible:outline-none"
                        >
                            <ProviderLogo provider={p.id} name={p.name} {...logoOf(p)} />
                            <span className="min-w-0 flex-1 truncate type-ui-medium text-fg">{p.name}</span>
                            <span className="truncate type-small text-faint">{sub(p)}</span>
                            <Icon name={open === p.id ? 'chevron-down' : 'chevron-right'} size={14} tone="muted" />
                        </button>
                        {open === p.id && (
                            <div className="border-t border-border bg-surface px-14 py-16">
                                <ConnectForm
                                    key={p.id}
                                    provider={p}
                                    onCancel={() => setOpen(null)}
                                    onDone={() => {
                                        setOpen(null)
                                        onConnected(p)
                                    }}
                                />
                            </div>
                        )}
                    </div>
                ))}
                {shown.length === 0 && (
                    <div className="flex flex-col items-start gap-8 p-16">
                        <p className="type-small text-muted">{q ? <>No provider matches “{query.trim()}”.</> : 'Nothing in this kind.'}</p>
                        {q && (
                            <Button icon="plus" onClick={() => onCustom(query.trim())}>
                                Create custom provider “{query.trim()}”
                            </Button>
                        )}
                    </div>
                )}
            </div>

            {!page && (
            <div className="flex flex-wrap items-center gap-12">
                <Button icon="plus" onClick={() => onCustom()}>
                    Add custom provider
                </Button>
                <p className="type-small text-faint">Any OpenAI-compatible or Anthropic-protocol endpoint, with its own model list.</p>
            </div>
            )}
        </div>
    )
}
