import { useEffect } from 'react'
import { create } from 'zustand'
import { listCatalogueProviders, listCatalogueModels } from '../services/vizier'
import type { CatalogueModel, CatalogueProvider } from '../interfaces/types'

/** Every picker on a page shares one provider list and one model list per provider. */
interface CatalogueStore {
  providers: CatalogueProvider[] | null
  providersLoading: boolean
  /** Keyed `${provider}:${all}`. */
  models: Record<string, CatalogueModel[] | undefined>
  modelErrors: Record<string, string | undefined>
  loadProviders: (force?: boolean) => Promise<void>
  loadModels: (provider: string, all: boolean, force?: boolean) => Promise<void>
  /** After connecting or editing a provider: refetch what the pickers show. */
  invalidate: () => Promise<void>
}

const inflight = new Map<string, Promise<void>>()

export const useCatalogueStore = create<CatalogueStore>()((set, get) => ({
  providers: null,
  providersLoading: false,
  models: {},
  modelErrors: {},
  loadProviders: async (force = false) => {
    if (!force && (get().providers || inflight.has('providers'))) return inflight.get('providers')
    const run = (async () => {
      set({ providersLoading: true })
      try {
        const res = await listCatalogueProviders()
        set({ providers: res.data || [] })
      } catch {
        set({ providers: get().providers || [] })
      } finally {
        set({ providersLoading: false })
        inflight.delete('providers')
      }
    })()
    inflight.set('providers', run)
    return run
  },
  loadModels: async (provider, all, force = false) => {
    const key = `${provider}:${all}`
    if (!provider) return
    if (!force && (get().models[key] || inflight.has(key))) return inflight.get(key)
    const run = (async () => {
      try {
        const res = await listCatalogueModels(provider, all)
        set((s) => ({
          models: { ...s.models, [key]: res.data || [] },
          modelErrors: { ...s.modelErrors, [key]: undefined },
        }))
      } catch {
        set((s) => ({
          models: { ...s.models, [key]: [] },
          modelErrors: { ...s.modelErrors, [key]: 'Could not load models' },
        }))
      } finally {
        inflight.delete(key)
      }
    })()
    inflight.set(key, run)
    return run
  },
  invalidate: async () => {
    inflight.clear()
    set({ models: {}, modelErrors: {} })
    await get().loadProviders(true)
  },
}))

export function useCatalogueProviders(): CatalogueProvider[] | null {
  const providers = useCatalogueStore((s) => s.providers)
  const load = useCatalogueStore((s) => s.loadProviders)
  useEffect(() => {
    load()
  }, [load])
  return providers
}

export function useCatalogueModels(provider: string, all: boolean): CatalogueModel[] | undefined {
  const models = useCatalogueStore((s) => s.models[`${provider}:${all}`])
  const load = useCatalogueStore((s) => s.loadModels)
  // Also after `invalidate` drops the cached list.
  const missing = models === undefined
  useEffect(() => {
    if (provider && missing) load(provider, all)
  }, [provider, all, load, missing])
  return models
}

/**
 * What the catalogue knows about one `(provider, model)`: `undefined` while loading, `null`
 * for a model it doesn't list.
 */
export function useModelInfo(provider: string, model: string): CatalogueModel | null | undefined {
  const all = useCatalogueModels(provider, true)
  if (!provider || !model) return null
  if (all === undefined) return undefined
  return all.find((m) => m.id === model) ?? null
}

/** `200000` → `200K`, `1048576` → `1M`. */
export function formatTokens(n: number | null | undefined): string {
  if (n == null) return '—'
  if (n >= 1_000_000) return `${+(n / 1_000_000).toFixed(n % 1_000_000 ? 1 : 0)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(n)
}

/** `$3 / $15` per million input / output tokens. */
export function formatPrice(cost: CatalogueModel['cost']): string | null {
  if (!cost) return null
  const f = (v: number) => (v === 0 ? '0' : v < 0.1 ? v.toFixed(3) : +v.toFixed(2) + '')
  return `$${f(cost.input)} / $${f(cost.output)}`
}
