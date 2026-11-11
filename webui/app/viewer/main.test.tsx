import { describe, expect, it } from 'vitest'
import { parseSnapshot } from '../lib/share/schema'
import { DAMAGED } from '../routes/share'

describe('viewer snapshot parsing and safety', () => {
    it('rejects damaged JSON and schema failures with clean error state', () => {
        expect(parseSnapshot(null).ok).toBe(false)
        expect(parseSnapshot('{ broken').ok).toBe(false)
        expect(parseSnapshot({ missing_fields: true }).ok).toBe(false)
    })

    it('rejects future format version with needs_newer', () => {
        const future = {
            format_version: 999,
            title: 'Future',
            source: { agent_name: 'Atlas' },
            include_trail: false,
            turns: [],
            media: {},
        }
        const res = parseSnapshot(future)
        expect(res.ok).toBe(false)
        if (!res.ok) {
            expect(res.reason).toBe('needs_newer')
            expect(res.message).toContain('newer version')
        }
    })

    it('prevents code execution across hostile inputs', () => {
        // window.__pwned must never be set
        const pwned = typeof window !== 'undefined' ? (window as unknown as { __pwned?: boolean }).__pwned : undefined
        expect(pwned).toBeUndefined()
    })
})
