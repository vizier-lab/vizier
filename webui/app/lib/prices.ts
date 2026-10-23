import { create } from 'zustand'
import { api } from '../api/client'
import type { VizierResponseStats } from '../api/frames'

export interface Price {
    input: number
    output: number
    cache_read?: number | null
    cache_write?: number | null
}

/** USD for a reply's token counts at `price` (per million tokens). */
export function estimateCost(
    stats: Pick<VizierResponseStats, 'total_input_tokens' | 'total_cached_input_tokens' | 'total_cache_creation_input_tokens' | 'total_output_tokens'>,
    price: Price
): number {
    const cached = stats.total_cached_input_tokens
    const written = stats.total_cache_creation_input_tokens
    const fresh = Math.max(0, stats.total_input_tokens - cached - written)
    const usd =
        fresh * price.input +
        cached * (price.cache_read ?? price.input) +
        written * (price.cache_write ?? price.input) +
        stats.total_output_tokens * price.output
    return usd / 1_000_000
}

type PriceStore = {
    prices: Record<string, Price | null>
    /** Fetches a model's price once; `null` means the catalogue has none. */
    load: (provider: string, model: string) => void
}

const pending = new Set<string>()

export const usePrices = create<PriceStore>()((set, get) => ({
    prices: {},
    load: (provider, model) => {
        const key = `${provider}/${model}`
        if (key in get().prices || pending.has(key)) return
        pending.add(key)
        api.get<{ cost?: Price | null }>(`/catalogue/providers/${encodeURIComponent(provider)}/models/${encodeURIComponent(model)}`)
            .then((m) => set((s) => ({ prices: { ...s.prices, [key]: m?.cost ?? null } })))
            .catch(() => set((s) => ({ prices: { ...s.prices, [key]: null } })))
            .finally(() => pending.delete(key))
    },
}))
