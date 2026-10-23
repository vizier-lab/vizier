import { Badge, Button, SearchField, SlideOver, Skeleton } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import type { ModelSummary } from '../../api/endpoints/catalogue'
import { allModels, fetchCustomModels } from '../../api/endpoints/providers'
import { capabilityLine } from '../../lib/models'
import { toast } from '../../stores/toasts'

type Props = {
    /** The provider whose models are shown; null closes the drawer. */
    provider: { id: string; name: string; custom: boolean } | null
    onClose: () => void
    /** After models were fetched from a custom provider's server. */
    onFetched: () => void
}

/** Every model a provider offers, with what it can do and costs. */
export function ModelsDrawer({ provider, onClose, onFetched }: Props) {
    const [models, setModels] = useState<ModelSummary[]>()
    const [query, setQuery] = useState('')
    const [fetching, setFetching] = useState(false)

    useEffect(() => {
        setModels(undefined)
        setQuery('')
        if (!provider) return
        let live = true
        allModels(provider.id).then(
            (m) => live && setModels(m ?? []),
            () => live && setModels([])
        )
        return () => {
            live = false
        }
    }, [provider])

    const shown = useMemo(() => {
        const q = query.trim().toLowerCase()
        return (models ?? []).filter((m) => !q || m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q))
    }, [models, query])

    return (
        <SlideOver
            open={provider !== null}
            title={provider ? `${provider.name} models` : ''}
            onClose={onClose}
            footer={
                provider?.custom ? (
                    <Button
                        loading={fetching}
                        onClick={async () => {
                            setFetching(true)
                            try {
                                await fetchCustomModels(provider.id)
                                setModels(await allModels(provider.id))
                                onFetched()
                            } catch (e) {
                                toast.error('Couldn’t fetch the model list', e instanceof Error ? e.message : undefined)
                            } finally {
                                setFetching(false)
                            }
                        }}
                    >
                        Fetch models from the server
                    </Button>
                ) : undefined
            }
        >
            <div className="flex flex-col gap-12">
                <SearchField aria-label="Search models" placeholder="Search models" value={query} onValueChange={setQuery} count={models ? `${shown.length}` : undefined} />
                {!models ? (
                    <Skeleton className="h-120" />
                ) : shown.length === 0 ? (
                    <p className="type-small text-faint">{models.length === 0 ? 'No models are listed.' : 'Nothing matched.'}</p>
                ) : (
                    <ul className="flex flex-col divide-y divide-border">
                        {shown.map((m) => (
                            <li key={m.id} className="flex flex-col gap-2 py-10">
                                <span className="flex items-center gap-8">
                                    <span className="type-code text-fg">{m.id}</span>
                                    {m.status !== 'active' && <Badge>{m.status}</Badge>}
                                </span>
                                <span className="type-small text-faint">{capabilityLine(m)}</span>
                            </li>
                        ))}
                    </ul>
                )}
            </div>
        </SlideOver>
    )
}
