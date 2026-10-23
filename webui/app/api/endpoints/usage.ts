import { api } from '../client'

export type Activity = 'chat' | 'background' | 'scheduled' | 'dreams' | 'compaction' | 'vision' | 'other'

export interface Breakdown {
    key: string
    label: string
    tokens: number
    calls: number
    cost_usd?: number | null
    detail?: string | null
    share?: number | null
}

export interface ModelRowDto extends Breakdown {
    provider: string
    model: string
    served_models: string[]
    failed_tokens: number
    priced: boolean
    cache_hit_rate?: number | null
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
}

export interface DailyUsage {
    date: string
    by_activity: Partial<Record<Activity, { calls: number; cost_usd?: number | null; tokens: number }>>
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
    summary: Summary
    previous?: { calls: number; cost_usd?: number | null; tokens: number } | null
    daily: DailyUsage[]
    by_activity: Breakdown[]
    by_model: ModelRowDto[]
    by_place: Breakdown[]
    costliest: CostlyConversation[]
    unpriced: { models: string[]; tokens: number }
}

export interface ConversationUsage {
    label: string
    detail?: string | null
    insight?: string | null
    summary: Summary
    calls: { activity: Activity; at: string; model: string; provider: string; input_tokens: number; output_tokens: number; cached_input_tokens: number; cost_usd?: number | null; failed: boolean; round?: number | null }[]
    more: number
    link?: { kind: string; path: string } | null
}

export const usageReport = (agentId: string, start: Date, end: Date, activity?: Activity) =>
    api.get<UsageReport>(`/agents/${agentId}/usage?start_date=${start.toISOString()}&end_date=${end.toISOString()}${activity ? `&activity=${activity}` : ''}`)

export const conversationUsage = (agentId: string, row: CostlyConversation, start: Date, end: Date) =>
    api.get<ConversationUsage>(
        `/agents/${agentId}/usage/conversation?channel=${encodeURIComponent(row.channel)}${row.topic ? `&topic=${encodeURIComponent(row.topic)}` : ''}&start_date=${start.toISOString()}&end_date=${end.toISOString()}&limit=50`
    )
