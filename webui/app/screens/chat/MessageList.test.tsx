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

    it('draws a skill as the message typed, with the /word highlighted and no chip', () => {
        const html = draw([
            entry('req', {
                Request: { timestamp: '2026-10-23T10:00:00Z', user: 'me', content: { chat: '/code-review the queue' }, metadata: { invoked_skill: 'code-review' } },
            } as ChatMessage['content']),
        ])
        expect(html).toContain('<span class="text-live-text">/code-review</span>')
        expect(html).toContain(' the queue')
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

describe('a message the turn read while it worked (spec 062)', () => {
    const report = JSON.stringify({ ok: true, result: 1, stdout: '', tool_calls: [], duration_ms: 3 })
    const finished: ChatMessage[] = [
        entry('req', { Request: { timestamp: '2026-10-23T10:00:00Z', user: 'me', content: { chat: 'refactor the auth module' } } }),
        entry('c1', { ToolCall: { call_id: 'c1', name: 'execute_python', arguments: { intent: 'first', code: '1' } } }, '2026-10-23T10:00:01Z'),
        entry('r1', { ToolResult: { call_id: 'c1', content: report } }, '2026-10-23T10:00:04Z'),
        // Sent at :02, read at :04: the row's time is where it was read, its request keeps when it was sent.
        entry('steer', { Request: { timestamp: '2026-10-23T10:00:02Z', user: 'me', content: { chat: 'keep the old token format' }, mid_turn: true } }, '2026-10-23T10:00:04Z'),
        entry('c2', { ToolCall: { call_id: 'c2', name: 'execute_python', arguments: { intent: 'second', code: '2' } } }, '2026-10-23T10:00:05Z'),
        entry('r2', { ToolResult: { call_id: 'c2', content: report } }, '2026-10-23T10:00:08Z'),
        entry('end', { Response: { timestamp: '2026-10-23T10:00:09Z', content: { message: { content: 'All done.' } } } }, '2026-10-23T10:00:09Z'),
    ]

    it('draws trail, the message, trail, then the one reply', () => {
        const html = draw(finished)
        const worked = [...html.matchAll(/Worked/g)].map((m) => m.index)
        expect(worked).toHaveLength(2)
        const message = html.indexOf('keep the old token format')
        expect(message).toBeGreaterThan(worked[0]!)
        expect(worked[1]!).toBeGreaterThan(message)
        expect(html.indexOf('All done.')).toBeGreaterThan(worked[1]!)
        expect(html.match(/All done\./g) ?? []).toHaveLength(1)
    })

    it('starts the part after the message with the agent, as the turn starts', () => {
        const html = draw(finished)
        const heads = [...html.matchAll(/>Oz</g)].map((m) => m.index!)
        expect(heads).toHaveLength(2)
        const message = html.indexOf('keep the old token format')
        expect(heads[0]).toBeLessThan(message)
        expect(heads[1]).toBeGreaterThan(message)
        expect(heads[1]).toBeLessThan(html.lastIndexOf('Worked'))
        // The second head says when the agent read the message (10:00:04), the row's own time.
        const second = html.slice(heads[1], html.lastIndexOf('Worked'))
        expect(second).toMatch(/type-mono-xs text-faint">\d{2}:\d{2}</)
    })

    it('draws the message once, inside the turn, after the request that started it', () => {
        // The prompt ruler names the current prompt in its accessible value; that is not drawn.
        const html = draw(finished).replace(/aria-valuetext="[^"]*"/g, '')
        expect(html.match(/keep the old token format/g) ?? []).toHaveLength(1)
        expect(html.indexOf('keep the old token format')).toBeGreaterThan(html.indexOf('refactor the auth module'))
    })
})

describe('a turn that ends on a question card', () => {
    const asked: ChatMessage[] = [
        entry('req', { Request: { timestamp: '2026-10-23T10:00:00Z', user: 'me', content: { chat: 'get me ready' } } }),
        entry('end', { Response: { timestamp: '2026-10-23T10:00:05Z', content: { message: { content: '', stats: { duration: { secs: 5, nanos: 0 }, total_input_tokens: 1, total_output_tokens: 1 } as never } } } }, '2026-10-23T10:00:05Z'),
    ]

    // The card is what the agent said; the reactions and the model line close the turn under it.
    it('draws the card above the reply footer', () => {
        const html = renderToString(
            <MessageList
                state={{ ...emptyTopic(session), entries: asked }}
                agent={{ id: 'oz', name: 'Oz' }}
                hasOlder={false}
                loadingOlder={false}
                onLoadOlder={() => {}}
                onReact={() => {}}
                questions={() => <div>the question card</div>}
            />
        )
        expect(html).toContain('the question card')
        expect(html.indexOf('in ·')).toBeGreaterThan(html.indexOf('the question card'))
    })
})
