import { createContext } from 'react'

/** The conversation a trail is drawn in, so a delivered script's row can read its record. */
export const DetachedSource = createContext<{ agentId: string; topicId: string } | null>(null)
