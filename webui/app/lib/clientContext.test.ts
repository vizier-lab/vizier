import { describe, expect, it } from 'vitest'
import { CLIENT_CONTEXT, withClientExtras } from './clientContext'
import { registerClientTool } from './clientTools'

/** A request frame as the stores build one. */
const request = (content: object = { chat: 'hi' }) => ({ timestamp: 'now', user: 'oz', content, metadata: null })

describe('client context', () => {
    it('rides every request, so the agent knows what this surface renders', () => {
        const frame = withClientExtras(request()) as { client_context?: string[] }
        expect(frame.client_context).toEqual(CLIENT_CONTEXT)
        expect(CLIENT_CONTEXT.join(' ')).toContain('mermaid')
        expect(CLIENT_CONTEXT.join(' ')).toContain('LaTeX')
    })

    // The engine refuses a request past its caps (`invalid_client_context`), so the client has to
    // stay inside them: 8 notes of 512 bytes each.
    it('stays inside the caps the engine enforces', () => {
        expect(CLIENT_CONTEXT.length).toBeLessThanOrEqual(8)
        for (const note of CLIENT_CONTEXT) expect(new TextEncoder().encode(note).length).toBeLessThanOrEqual(512)
    })

    it('is left off a command, a reaction or a tool result', () => {
        for (const frame of [{ content: { command: 'abort' } }, { reaction: { emoji: 'x' } }, { client_tool_result: { call_id: 'c' } }, { content: null }]) {
            expect(withClientExtras(frame)).toBe(frame)
        }
    })

    // Declaring tools and declaring what this client renders are one step now; neither may drop
    // the other.
    it('carries the declared tools alongside it', () => {
        const off = registerClientTool({
            definition: { name: 'pick_color', description: 'Pick one', input_schema: { type: 'object' } },
            handle: () => 'red',
        })
        try {
            const frame = withClientExtras(request()) as { client_tools?: { name: string }[]; client_context?: string[] }
            expect(frame.client_tools?.map((tool) => tool.name)).toEqual(['pick_color'])
            expect(frame.client_context).toEqual(CLIENT_CONTEXT)
        } finally {
            off()
        }
    })
})
