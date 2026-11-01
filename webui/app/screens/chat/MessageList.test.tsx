import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { ChatMessage } from '../../api/frames'
import { emptyTopic } from '../../lib/frames'
import { MessageList } from './MessageList'

vi.mock('../../api/transport', () => ({ transport: { authMode: 'http' } }))

const session = { agent_id: 'oz', channel: 'vizier-webui', topic: 't' }

const entry = (uid: string, content: ChatMessage['content'], timestamp = '2026-10-23T10:00:00Z'): ChatMessage => ({
    uid,
    timestamp,
    vizier_session: session,
    content,
})

const draw = (entries: ChatMessage[]) => {
    const state = { ...emptyTopic(session), entries }
    return renderToString(
        <MessageList
            state={state}
            agent={{ id: 'oz', name: 'Oz' }}
            hasOlder={false}
            loadingOlder={false}
            onLoadOlder={() => {}}
            onReact={() => {}}
        />
    )
}

describe('the divider a reset leaves behind', () => {
    it('draws a checkpoint and says whether it handed anything over', () => {
        expect(draw([entry('cp', { Checkpoint: 'the handover' })])).toContain('Checkpoint · handed over')
    })

    it('says a checkpoint with no handover left nothing to open', () => {
        const html = draw([entry('cp', { Checkpoint: { handover: null, timestamp: '2026-10-23T10:00:00Z' } })])
        expect(html).toContain('Checkpoint · no handover')
        expect(html).not.toContain('handed over')
    })
})

describe('a command the person ran', () => {
    it('is written out, so /lobotomy is not just a bare divider', () => {
        const html = draw([
            entry('cmd', { Command: 'lobotomy' }),
            entry('cp', { Checkpoint: { handover: null, timestamp: '2026-10-23T10:00:01Z' } }, '2026-10-23T10:00:01Z'),
        ])
        expect(html).toContain('lobotomy')
        expect(html).toContain('text-live-text')
    })

    it('is written out for /checkpoint too', () => {
        const html = draw([
            entry('cmd', { Command: 'checkpoint' }),
            entry('cp', { Checkpoint: 'the handover' }, '2026-10-23T10:00:01Z'),
        ])
        expect(html).toContain('text-live-text')
    })

    it('does not repeat /abort, which the StoppedNote already names', () => {
        const html = draw([
            entry('req', { Request: { timestamp: '2026-10-23T10:00:00Z', user: 'me', content: { chat: 'hi' } } }),
            entry('cmd', { Command: 'abort' }, '2026-10-23T10:00:01Z'),
        ])
        expect(html).not.toContain('text-live-text')
    })
})
