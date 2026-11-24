import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { entryDisposition, HISTORY_KINDS, included, requestDisposition, responseDisposition, dispositionOf } from './kinds'

const read = (rel: string) => readFileSync(join(__dirname, '..', '..', rel), 'utf8')

/** Keys of `ChatMessage.content` in frames.ts, and the variants of `SessionHistoryContent` in the generated schema. */
function frameKinds(): string[] {
    const src = read('api/frames.ts')
    const start = src.indexOf('export interface ChatMessage')
    const block = src.slice(src.indexOf('content: {', start), src.indexOf('reactions?', start))
    return [...block.matchAll(/^\s{4}([A-Z][A-Za-z]+)\??:/gm)].map((m) => m[1])
}
function schemaKinds(): string[] {
    const src = read('api/schema.d.ts')
    const start = src.indexOf('SessionHistoryContent: {')
    const end = src.indexOf('SetGrants:', start)
    return [...src.slice(start, end).matchAll(/^ {12}([A-Z][A-Za-z]+): /gm)].map((m) => m[1])
}

describe('kinds table', () => {
    it('lists every history variant frames.ts knows', () => {
        const known = frameKinds()
        expect(known.length).toBeGreaterThanOrEqual(10)
        expect(known.filter((k) => !(k in HISTORY_KINDS))).toEqual([])
    })
    it('lists every variant of the generated SessionHistoryContent', () => {
        const known = schemaKinds()
        expect(known.length).toBeGreaterThanOrEqual(10)
        expect(known.filter((k) => !(k in HISTORY_KINDS))).toEqual([])
    })
    it('never shares recall, sensitive asks, checkpoints or commands', () => {
        for (const k of ['Recall', 'SensitiveAsk', 'Checkpoint', 'Command']) expect(HISTORY_KINDS[k]).toBe('never')
    })
    it('treats unknown kinds as never, including prototype names', () => {
        expect(dispositionOf(HISTORY_KINDS, 'BrandNew')).toBe('never')
        expect(dispositionOf(HISTORY_KINDS, 'constructor')).toBe('never')
        expect(dispositionOf(HISTORY_KINDS, '__proto__')).toBe('never')
        expect(entryDisposition({ BrandNew: 1 }).disposition).toBe('never')
        expect(entryDisposition({}).disposition).toBe('never')
        expect(entryDisposition(null).disposition).toBe('never')
    })
    it('splits by the trail switch', () => {
        expect(included('trail-off', false)).toBe(true)
        expect(included('trail-on', false)).toBe(false)
        expect(included('trail-on', true)).toBe(true)
        expect(included('never', true)).toBe(false)
    })
    it('refines requests, responses and client calls', () => {
        expect(requestDisposition({ chat: 'x' })).toBe('trail-off')
        expect(requestDisposition({ silent_read: 'x' })).toBe('never')
        expect(requestDisposition({ background_report: {} })).toBe('never')
        expect(responseDisposition({ message: { content: 'x' } })).toBe('trail-off')
        expect(responseDisposition({ thinking: 'x' })).toBe('never')
        expect(responseDisposition('thinking_start')).toBe('never')
        expect(responseDisposition({ tool_choice: {} })).toBe('trail-on')
        expect(entryDisposition({ ClientToolCall: { name: 'ui_render' } }).disposition).toBe('trail-off')
        expect(entryDisposition({ ClientToolCall: { name: 'other_tool' } }).disposition).toBe('trail-on')
        expect(entryDisposition({ ToolCall: {} }).disposition).toBe('trail-on')
    })
})
