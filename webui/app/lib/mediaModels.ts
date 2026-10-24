import type { MediaModels, MediaModelSummary } from '../api/endpoints/catalogue'

/** A model row, or the row a typed id becomes when it is not in the list. */
export type ModelRow = MediaModelSummary & { typed?: boolean }

/** A voice row, or the row a typed voice becomes. */
export type VoiceRow = { id: string; typed?: boolean }

export type Section<T> = { label?: string; items: T[] }

const needle = (query: string) => query.trim().toLowerCase()

/** Models whose id or readable name contains the query. */
export function filterModels(models: MediaModelSummary[], query: string): MediaModelSummary[] {
    const q = needle(query)
    if (!q) return models
    return models.filter((m) => m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q))
}

/**
 * The `Use “…”` row for an id that is not in the list, so a model that is missing (retired,
 * newly released, or an unreadable list) can still be chosen (FR-021).
 */
export function typedModel(query: string, models: MediaModelSummary[]): ModelRow | undefined {
    const id = query.trim()
    if (!id || models.some((m) => m.id === id)) return undefined
    return {
        id,
        name: `Use “${id}”`,
        default: false,
        listed: false,
        voices: [],
        aspect_ratios: [],
        resolutions: [],
        typed: true,
    }
}

/**
 * The picker's sections: the typed row first (it is what the query just created), then the
 * documented default on its own, then the rest, matching name or id.
 */
export function modelSections(
    models: MediaModelSummary[] | undefined,
    query: string
): Section<ModelRow>[] {
    const list = models ?? []
    const matching = filterModels(list, query)
    const defaultModel = matching.find((m) => m.default)
    const others = matching.filter((m) => !m.default)
    const typed = typedModel(query, list)
    return [
        { label: 'Typed id', items: typed ? [typed] : [] },
        { label: 'Default', items: defaultModel ? [defaultModel] : [] },
        { label: 'Models', items: others },
    ]
}

export function selectedModel(
    models: MediaModelSummary[] | undefined,
    id: string
): MediaModelSummary | undefined {
    return models?.find((m) => m.id === id)
}

/**
 * The documented default: the one the listing marks, or — before the list has loaded — the
 * local defaults map's id, shown as `Default · <id>` (FR-011).
 */
export function defaultModelOf(
    models: MediaModelSummary[] | undefined,
    fallbackId: string
): MediaModelSummary | undefined {
    return (
        models?.find((m) => m.default) ??
        (fallbackId
            ? {
                  id: fallbackId,
                  name: fallbackId,
                  default: true,
                  listed: false,
                  voices: [],
                  aspect_ratios: [],
                  resolutions: [],
              }
            : undefined)
    )
}

export type TriggerLabel = { name: string; id?: string; isDefault: boolean }

/**
 * What the trigger shows: the chosen model's `Name · id`, or `Default · <name>` when the field
 * is empty and the documented default is the effective value.
 */
export function triggerLabel(
    models: MediaModelSummary[] | undefined,
    model: string,
    fallbackDefault: string
): TriggerLabel {
    if (model) {
        const found = selectedModel(models, model)
        const name = found?.name ?? model
        return { name, id: name !== model ? model : undefined, isDefault: false }
    }
    const fallback = defaultModelOf(models, fallbackDefault)
    const name = fallback?.name ?? ''
    return { name, id: name && name !== fallback?.id ? fallback?.id : undefined, isDefault: true }
}

/** The voices the selected speech model publishes; an empty selection means the default model. */
export function voicesOf(models: MediaModelSummary[] | undefined, model: string): string[] {
    const selected = model ? selectedModel(models, model) : models?.find((m) => m.default)
    return selected?.voices ?? []
}

export function typedVoice(query: string, voices: string[]): VoiceRow | undefined {
    const id = query.trim()
    if (!id || voices.includes(id)) return undefined
    return { id, typed: true }
}

/** The voice picker's sections: typed first, the documented default, then the model's voices. */
export function voiceSections(
    voices: string[],
    query: string,
    defaultVoice: string
): Section<VoiceRow>[] {
    const q = needle(query)
    const matching = voices.filter((v) => !q || v.toLowerCase().includes(q))
    const isDefault = (v: string) => defaultVoice !== '' && v === defaultVoice
    const typed = typedVoice(query, voices)
    return [
        { label: 'Typed voice', items: typed ? [typed] : [] },
        { label: 'Default', items: matching.filter(isDefault).map((id) => ({ id })) },
        { label: 'Voices', items: matching.filter((v) => !isDefault(v)).map((id) => ({ id })) },
    ]
}

/** The default voice the field falls back to: the server's once loaded, else the local map. */
export function defaultVoiceOf(data: MediaModels | undefined, fallback: string): string {
    return data?.default_voice ?? fallback
}

/** "Cached · 2 d old" for the picker's status line. */
export function sourceLine(data: MediaModels | undefined, now: number = Date.now()): string | undefined {
    if (!data) return undefined
    const source = data.source === 'cached' ? 'Cached' : 'Bundled'
    const age = ageOf(data.fetched_at, now)
    return age ? `${source} · ${age}` : source
}

export function ageOf(iso: string, now: number = Date.now()): string | undefined {
    const then = Date.parse(iso)
    if (Number.isNaN(then)) return undefined
    const seconds = Math.max(0, Math.round((now - then) / 1000))
    if (seconds < 60) return 'just now'
    const minutes = Math.round(seconds / 60)
    if (minutes < 60) return `${minutes} min old`
    const hours = Math.round(minutes / 60)
    if (hours < 24) return `${hours} h old`
    return `${Math.round(hours / 24)} d old`
}
