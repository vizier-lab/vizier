import { renderToString } from 'react-dom/server'
import { StaticRouter } from 'react-router'
import { describe, expect, it } from 'vitest'
import { ApiError } from '../api/client'
import { Neutral, RATE_LIMITED, UNAVAILABLE, viewFor } from './share'

const doc = (over: Record<string, unknown> = {}) => ({ format_version: 1, created_at: 1, title: 'T', source: { agent_name: 'A' }, include_trail: false, turns: [], media: {}, rehydration: null, ...over })

describe('share route states', () => {
    it('opens a valid snapshot', () => {
        const v = viewFor({ data: { snapshot: doc() } })
        expect(v.state).toBe('ok')
    })
    it('401 asks to sign in; 404 and 429 are neutral without a sign-in button', () => {
        expect(viewFor({ error: new ApiError(401, 'sign_in_required', 'x') })).toEqual({ state: 'unavailable', signIn: true })
        expect(viewFor({ error: new ApiError(404, 'share_unavailable', 'x') })).toEqual({ state: 'unavailable', signIn: false })
        expect(viewFor({ error: new ApiError(429, 'rate_limited', 'x') })).toEqual({ state: 'unavailable', signIn: false, message: RATE_LIMITED })
        expect(viewFor({ error: new TypeError('network') })).toEqual({ state: 'unavailable', signIn: false })
    })
    it('a damaged or newer document is not opened', () => {
        expect(viewFor({ data: { snapshot: 'nope' } }).state).toBe('damaged')
        expect(viewFor({ data: { snapshot: doc({ turns: 5 }) } }).state).toBe('damaged')
        expect(viewFor({ data: { snapshot: doc({ format_version: 2 }) } }).state).toBe('needs_newer')
        expect(viewFor({ data: null }).state).toBe('damaged')
    })
    it('Neutral component renders sign in button with returnTo for 401', () => {
        const html = renderToString(
            <StaticRouter location="/share/test-share-123">
                <Neutral message={UNAVAILABLE} signIn shareId="test-share-123" />
            </StaticRouter>
        )
        expect(html).toContain('Sign in')
        expect(html).toContain('href="/login?returnTo=%2Fshare%2Ftest-share-123"')
    })
    it('Neutral component renders message without sign in button for 404 and 429', () => {
        const html404 = renderToString(
            <StaticRouter location="/share/test-share-123">
                <Neutral message={UNAVAILABLE} signIn={false} shareId="test-share-123" />
            </StaticRouter>
        )
        expect(html404).toContain('This link')
        expect(html404).not.toContain('Sign in')

        const html429 = renderToString(
            <StaticRouter location="/share/test-share-123">
                <Neutral message={RATE_LIMITED} signIn={false} shareId="test-share-123" />
            </StaticRouter>
        )
        expect(html429).toContain('Try again later')
        expect(html429).not.toContain('Sign in')
    })
})
