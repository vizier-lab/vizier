/**
 * What the app remembers between pages and reloads, per browser. Every read
 * falls back when storage is unavailable (a private window, blocked site data),
 * and a write that fails is simply not remembered.
 */

export function read<T>(key: string, fallback: T): T {
    try {
        const v = localStorage.getItem(key)
        return v ? (JSON.parse(v) as T) : fallback
    } catch {
        return fallback
    }
}

export function write(key: string, value: unknown) {
    try {
        localStorage.setItem(key, JSON.stringify(value))
    } catch {
        /* not remembered */
    }
}

const topicKey = (userId: string, agentId: string) => `vizier.chat.topic.${userId}.${agentId}`

/** The topic this person last had open with this agent, sent to or not. */
export const lastTopic = (userId: string, agentId: string) => read<string | null>(topicKey(userId, agentId), null)

export const rememberTopic = (userId: string, agentId: string, topicId: string) => write(topicKey(userId, agentId), topicId)
