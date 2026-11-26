import { describe, expect, it } from 'vitest'
import type { ChatMessage } from '../api/frames'
import { requestText } from '../lib/messages'

const session = { agent_id: 'plug', channel: 'webui', topic: 't' }

const request = (content: ChatMessage['content']['Request'] extends { content: infer C } ? C : never): ChatMessage => ({
    uid: 'r1',
    timestamp: '2026-11-26T09:14:03Z',
    vizier_session: session,
    content: { Request: { timestamp: '2026-11-26T09:14:03Z', user: 'plug', content } },
})

describe('the woken request', () => {
    it('shows the work the agent was given', () => {
        expect(requestText({ woken: 'summarise the inbox' })).toBe('summarise the inbox')
    })

    it('is not the person\'s message: the frontmatter names the agent', () => {
        const entry = request({ woken: 'summarise the inbox' })
        const req = entry.content.Request!
        expect(req.user).toBe('plug')
        expect('woken' in req.content).toBe(true)
    })
})
