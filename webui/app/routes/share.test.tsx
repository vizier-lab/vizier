import { describe, expect, it } from 'vitest'
import { ApiError } from '../api/client'
import { viewFor } from './share'

const doc = (over: Record<string, unknown> = {}) => ({ format_version: 1, created_at: 1, title: 'T', source: { agent_name: 'A' }, include_trail: false, turns: [], media: {}, rehydration: null, ...over })

describe('share route states', () => {
    it('opens a valid snapshot', () => {
        const v = viewFor({ data: { snapshot: doc() } })
        expect(v.state).toBe('ok')
    })
    it('401 asks to sign in; 404 and 429 are neutral without a sign-in button', () => {
        expect(viewFor({ error: new ApiError(401, 'sign_in_required', 'x') })).toEqual({ state: 'unavailable', signIn: true })
        expect(viewFor({ error: new ApiError(404, 'share_unavailable', 'x') })).toEqual({ state: 'unavailable', signIn: false })
        expect(viewFor({ error: new ApiError(429, 'rate_limited', 'x') })).toEqual({ state: 'unavailable', signIn: false })
        expect(viewFor({ error: new TypeError('network') })).toEqual({ state: 'unavailable', signIn: false })
    })
    it('a damaged or newer document is not opened', () => {
        expect(viewFor({ data: { snapshot: 'nope' } }).state).toBe('damaged')
        expect(viewFor({ data: { snapshot: doc({ turns: 5 }) } }).state).toBe('damaged')
        expect(viewFor({ data: { snapshot: doc({ format_version: 2 }) } }).state).toBe('needs_newer')
        expect(viewFor({ data: null }).state).toBe('damaged')
    })
})
