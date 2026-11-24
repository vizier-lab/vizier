// A 401 on a share fetch goes through the real transport and the real session handler (T054).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { loginRedirectFor, TOKEN_KEY } from '../api/transport/common'
import { getShare } from '../api/shares'
import { useSession } from '../stores/session'
import { ApiError } from '../api/client'
import { viewFor } from './share'

const assign = vi.fn()
const stubLocation = (pathname: string) => vi.stubGlobal('location', { pathname, search: '', protocol: 'http:', host: 'x', assign })

const store = new Map<string, string>()
beforeEach(() => {
    assign.mockReset()
    store.clear()
    vi.stubGlobal('localStorage', {
        getItem: (k: string) => store.get(k) ?? null,
        setItem: (k: string, v: string) => void store.set(k, v),
        removeItem: (k: string) => void store.delete(k),
    })
    localStorage.setItem(TOKEN_KEY, 'stale')
    useSession.setState({ token: 'stale', user: null })
    vi.stubGlobal('fetch', async () => new Response(JSON.stringify({ status: 401, code: 'sign_in_required', message: 'Sign in' }), { status: 401 }))
})
afterEach(() => vi.unstubAllGlobals())

describe('401 on /share/:id', () => {
    it('does not hard-redirect to a bare /login, so the sign-in link keeps returnTo', async () => {
        stubLocation('/share/abc')
        const err = await getShare('abc').catch((e) => e)
        expect(err).toBeInstanceOf(ApiError)
        expect(assign).not.toHaveBeenCalled()
        // The page then offers its own sign-in link with the share path.
        expect(viewFor({ error: err })).toEqual({ state: 'unavailable', signIn: true })
        // The stale token is dropped either way.
        expect(localStorage.getItem(TOKEN_KEY)).toBeNull()
    })

    it('any other page still goes to /login', async () => {
        stubLocation('/settings/shares')
        await getShare('abc').catch(() => undefined)
        expect(assign).toHaveBeenCalledWith('/login')
    })

    it('never redirects from /login itself', () => {
        expect(loginRedirectFor('/login')).toBeNull()
        expect(loginRedirectFor('/share/x')).toBeNull()
        expect(loginRedirectFor('/sharex')).toBe('/login')
        expect(loginRedirectFor('/')).toBe('/login')
    })
})
