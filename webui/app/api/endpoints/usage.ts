import { api } from '../client'

export type Activity =
    | 'chat'
    | 'project'
    | 'background'
    | 'scheduled'
    | 'dreams'
    | 'compaction'
    | 'vision'
    | 'other'
    | 'consultation'
    | 'indexing'

/** What kind of model call (spec 065): every completion is `text`. */
export type UsageKind = 'text' | 'image' | 'speech' | 'transcription' | 'embedding'

/** A media amount in its kind's own unit; only the fields of the kinds in the row are set. */
export interface Units {
    images?: number
    characters?: number
    audio_seconds?: number
    tokens?: number
}

/**
 * Which source or rule priced a figure (spec 068): a catalogue source id (`models_dev`, `openrouter`,
 * `venice`, `curated`) or a rule (`inference_profile`, `local`, `subscription`, `borrowed`, `none`).
 * Open: a new source adds a value. A source's display name is in the report's `sources`.
 */
export type PriceSource = string

/** Another source that prices a model row differently, priced on the row's own amounts. */
export interface PriceAlternative {
    source: string
    cost_usd: number
}

/** A catalogue source the report's labels name. */
export interface SourceSummary {
    id: string
    name: string
    fetched_at: string
}

export interface Breakdown {
    key: string
    label: string
    tokens: number
    calls: number
    cost_usd?: number | null
    detail?: string | null
    share?: number | null
    units?: Units | null
}

export interface KindRowDto extends Breakdown {
    kind: UsageKind
}

export interface ModelRowDto extends Breakdown {
    provider: string
    model: string
    served_models: string[]
    failed_tokens: number
    priced: boolean
    cache_hit_rate?: number | null
    kind?: UsageKind
    price_source?: PriceSource
    borrowed_from?: string | null
    price_alternatives?: PriceAlternative[]
}

export interface Summary {
    tokens: number
    input_tokens: number
    cached_input_tokens: number
    cache_creation_input_tokens: number
    output_tokens: number
    calls: number
    cost_usd?: number | null
    cache_hit_rate?: number | null
    failed: { cost_usd?: number | null; tokens: number; turns: number }
    kind_share_of_all?: number | null
}

type DayCell = { calls: number; cost_usd?: number | null; tokens: number }

export interface DailyUsage {
    date: string
    by_activity: Partial<Record<Activity, DayCell>>
    by_kind?: Partial<Record<UsageKind, DayCell>>
}

export interface CostlyConversation {
    channel: string
    topic: string | null
    label: string
    detail?: string | null
    activity: Activity
    model: string
    tokens: number
    cost_usd?: number | null
    failed_share?: number | null
    last_at: string
    exists: boolean
}

export interface UsageReport {
    range: { start: string; end: string }
    accounting_since?: string | null
    media_accounting_since?: string | null
    kind?: UsageKind | null
    summary: Summary
    previous?: { calls: number; cost_usd?: number | null; tokens: number } | null
    daily: DailyUsage[]
    by_activity: Breakdown[]
    by_kind?: KindRowDto[]
    by_model: ModelRowDto[]
    by_place: Breakdown[]
    costliest: CostlyConversation[]
    unpriced: { models: string[]; tokens: number; units?: Partial<Record<UsageKind, Units>> }
    sources?: SourceSummary[]
}

/** One media record a tool call made, priced (spec 065, `GET …/usage/calls`). */
export interface MediaCall {
    kind: UsageKind
    provider: string
    model: string
    variant?: string | null
    units?: Units | null
    measured: boolean
    local: boolean
    cost_usd?: number | null
    price_source: PriceSource
    at: string
}

export interface ConversationUsage {
    label: string
    detail?: string | null
    insight?: string | null
    summary: Summary
    calls: {
        activity: Activity
        at: string
        model: string
        provider: string
        input_tokens: number
        output_tokens: number
        cached_input_tokens: number
        cost_usd?: number | null
        failed: boolean
        round?: number | null
        kind?: UsageKind
        variant?: string | null
        units?: Units | null
        measured?: boolean
    }[]
    more: number
    link?: { kind: string; path: string } | null
}

export const usageReport = (agentId: string, start: Date, end: Date, activity?: Activity, kind?: UsageKind) =>
    api.get<UsageReport>(
        `/agents/${agentId}/usage?start_date=${start.toISOString()}&end_date=${end.toISOString()}${activity ? `&activity=${activity}` : ''}${kind ? `&kind=${kind}` : ''}`
    )

export const conversationUsage = (agentId: string, row: CostlyConversation, start: Date, end: Date, kind?: UsageKind) =>
    api.get<ConversationUsage>(
        `/agents/${agentId}/usage/conversation?channel=${encodeURIComponent(row.channel)}${row.topic ? `&topic=${encodeURIComponent(row.topic)}` : ''}&start_date=${start.toISOString()}&end_date=${end.toISOString()}&limit=50${kind ? `&kind=${kind}` : ''}`
    )

/** The media calls each named `execute_python` call made (at most 50 ids). */
export const usageCalls = (agentId: string, callIds: string[]) =>
    api.get<{ calls: Record<string, MediaCall[]> }>(
        `/agents/${agentId}/usage/calls?ids=${callIds.map(encodeURIComponent).join(',')}`
    )
