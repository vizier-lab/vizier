import { read, write } from './remember'
import type { ModelOverride } from '../api/frames'

export type ModelOverridePreferenceKey = {
    userId: string
    agentId: string
    channelId: string
    topicId: string
}

const keyFor = ({ userId, agentId, channelId, topicId }: ModelOverridePreferenceKey) =>
    `vizier.chat.model.${[userId, agentId, channelId, topicId].map(encodeURIComponent).join('.')}`

const isChoice = (value: unknown): value is ModelOverride => {
    if (!value || typeof value !== 'object') return false
    const choice = value as Partial<ModelOverride>
    return typeof choice.provider === 'string' && typeof choice.model === 'string'
}

/** Browser-local choice for one user and one conversation. `null` means Agent default. */
export function readModelOverride(key: ModelOverridePreferenceKey): ModelOverride | null {
    const value = read<unknown>(keyFor(key), null)
    return value === null || isChoice(value) ? value : null
}

export function writeModelOverride(key: ModelOverridePreferenceKey, choice: ModelOverride | null) {
    write(keyFor(key), choice)
}

/** Exact optional field captured on one submitted user chat request. */
export function modelOverridePayload(choice: ModelOverride | null) {
    return choice ? { model_override: { provider: choice.provider, model: choice.model } } : {}
}
