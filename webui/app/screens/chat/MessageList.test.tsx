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

describe('a turn that spoke while it worked (spec 053, revised)', () => {
    const report = JSON.stringify({ ok: true, result: 1, stdout: '', tool_calls: [], duration_ms: 3 })
    const turn: ChatMessage[] = [
        entry('req', { Request: { timestamp: '2026-10-23T10:00:00Z', user: 'me', content: { chat: 'hi' } } }),
        entry('c1', { ToolCall: { call_id: 'c1', name: 'execute_python', arguments: { intent: 'first', code: '1' } } }, '2026-10-23T10:00:01Z'),
        entry('r1', { ToolResult: { call_id: 'c1', content: report } }, '2026-10-23T10:00:04Z'),
        entry('say', { AssistantMessage: 'The log points at the deploy.' }, '2026-10-23T10:00:05Z'),
        entry('c2', { ToolCall: { call_id: 'c2', name: 'execute_python', arguments: { intent: 'second', code: '2' } } }, '2026-10-23T10:00:06Z'),
        entry('r2', { ToolResult: { call_id: 'c2', content: report } }, '2026-10-23T10:00:08Z'),
    ]
    const finished = [
        ...turn,
        entry('end', { Response: { timestamp: '2026-10-23T10:00:09Z', content: { message: { content: 'All done.', stats: { duration: { secs: 9, nanos: 0 }, total_input_tokens: 1, total_output_tokens: 1 } as never } } } }, '2026-10-23T10:00:09Z'),
    ]

    // One trail for the whole turn: the prose is a step of it, not a second one below it.
    it('draws one trail for the turn, then the reply', () => {
        const html = draw(finished)
        expect(html.match(/Worked/g) ?? []).toHaveLength(1)
        expect(html.indexOf('All done.')).toBeGreaterThan(html.indexOf('Worked'))
    })

    it('keeps stats on the final message, with the whole time there', () => {
        const html = draw(finished)
        expect(html).toContain('9')
        expect(html.indexOf('in ·')).toBeGreaterThan(html.indexOf('All done.'))
    })

    // A finished turn folds its trail, so its prose is one click away; a running one shows it.
    it('shows the prose inside the open trail of a turn still running', () => {
        const state = { ...emptyTopic(session), entries: turn, phase: 'running' as never }
        const html = renderToString(
            <MessageList state={state} agent={{ id: 'oz', name: 'Oz' }} hasOlder={false} loadingOlder={false} onLoadOlder={() => {}} onReact={() => {}} />
        )
        // The running head names the newest script's intent, not an idle word.
        expect(html).not.toContain('Oz is ')
        const head = html.indexOf('>second<')
        expect(head).toBeGreaterThan(-1)
        expect(html.indexOf('The log points at the deploy.')).toBeGreaterThan(head)
    })
})
