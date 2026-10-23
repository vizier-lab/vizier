import { useEffect, useState } from 'react'
import { allProviders, providerModels, type ModelSummary, type ProviderSummary } from '../api/endpoints/catalogue'
import { allModels } from '../api/endpoints/providers'

type Listener = () => void

let providers: Promise<ProviderSummary[]> | undefined
let loaded: ProviderSummary[] | undefined
const models = new Map<string, Promise<ModelSummary[]>>()
const listeners = new Set<Listener>()

/** Every provider the catalogue lists, with its connection state. Loaded once; `invalidate` loads it again. */
export function loadProviders(): Promise<ProviderSummary[]> {
    providers ??= allProviders().then(
        (all) => {
            loaded = all ?? []
            listeners.forEach((l) => l())
            return loaded
        },
        (e) => {
            providers = undefined
            return Promise.reject(e)
        }
    )
    return providers
}

/** One provider's models: the listed ones, or with `all` the deprecated and no-tool-call ones too. */
export function loadModels(id: string, all = false): Promise<ModelSummary[]> {
    const key = `${id}${all ? ':all' : ''}`
    let hit = models.get(key)
    if (!hit) {
        hit = (all ? allModels(id) : providerModels(id)).then((m) => m ?? []).catch((e) => (models.delete(key), Promise.reject(e)))
        models.set(key, hit)
    }
    return hit
}

/** After a provider is connected, changed or removed: the next read asks the server again. */
export function invalidateCatalogue() {
    providers = undefined
    models.clear()
}

/** The provider list, once it has loaded; `reload` drops the cache and asks again. */
export function useCatalogueProviders() {
    const [list, setList] = useState<ProviderSummary[] | undefined>(loaded)
    const [failed, setFailed] = useState(false)
    useEffect(() => {
        let live = true
        const sync = () => live && loaded && setList(loaded)
        listeners.add(sync)
        loadProviders().then(
            (p) => live && setList(p),
            () => live && setFailed(true)
        )
        return () => {
            live = false
            listeners.delete(sync)
        }
    }, [])
    return {
        providers: list,
        failed,
        reload: async () => {
            invalidateCatalogue()
            setList(await loadProviders())
        },
    }
}

/** The props ProviderLogo / ProviderTile / ProviderRow take for a catalogue entry's logo. */
export const logoOf = (p?: { logo?: string | null; logo_full_color?: boolean }) => ({ src: p?.logo ?? undefined, fullColor: p?.logo_full_color })
