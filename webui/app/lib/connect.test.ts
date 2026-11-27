import { afterEach, describe, expect, it } from 'vitest'
import { ApiError, loginRedirectFor } from '../api/transport/common'
import {
    accountName,
    bareHandle,
    clearStashedCode,
    codeFromHash,
    connectedSince,
    isCodeShaped,
    normalizeCode,
    refusalOf,
    shownSender,
    stashCode,
    stashedCode,
} from './connect'

describe('connect codes', () => {
    it('ignores case, spaces and dashes', () => {
        expect(normalizeCode(' abcd-efgh ')).toBe('ABCDEFGH')
        expect(normalizeCode('ab cd ef gh')).toBe('ABCDEFGH')
        expect(isCodeShaped('abcd-efgh')).toBe(true)
        for (const input of ['', 'abcd', 'abcd-efgh-j', 'abcd_efgh']) expect(isCodeShaped(input)).toBe(false)
    })

    it('reads the code from a link fragment', () => {
        expect(codeFromHash('#WXYZ-KMNP')).toBe('WXYZKMNP')
        expect(codeFromHash('#wxyz%20kmnp')).toBe('WXYZKMNP')
        expect(codeFromHash('')).toBeNull()
        expect(codeFromHash('#section')).toBeNull()
        expect(codeFromHash('#%')).toBeNull()
    })
})

describe('the stashed code', () => {
    afterEach(clearStashedCode)

    it('waits through the sign-in and goes when cleared', () => {
        expect(stashedCode()).toBeNull()
        stashCode('ABCDEFGH')
        expect(stashedCode()).toBe('ABCDEFGH')
        clearStashedCode()
        expect(stashedCode()).toBeNull()
    })
})

describe('refusals', () => {
    it('maps the server codes to what is shown', () => {
        const unavailable = new ApiError(404, 'connect_code_unavailable', 'This code has expired.')
        expect(refusalOf(unavailable)).toEqual({ state: 'unavailable', message: 'This code has expired.' })
        expect(refusalOf(new ApiError(409, 'platform_linked_elsewhere', 'Taken.')).state).toBe('elsewhere')
        expect(refusalOf(new ApiError(429, 'rate_limited', 'Slow down.')).state).toBe('failed')
        expect(refusalOf(new Error('offline'))).toEqual({ state: 'failed', message: 'offline' })
    })
})

describe('signing in from /connect', () => {
    it('brings the person back to the confirmation', () => {
        expect(loginRedirectFor('/connect')).toBe('/login?returnTo=%2Fconnect')
    })
})

describe('connectedSince', () => {
    it('names the day', () => {
        expect(connectedSince('2026-11-27T12:00:00Z')).toMatch(/^Connected 2[678] Nov 2026$/)
    })
})

describe('who a platform message is shown as', () => {
    const request = (user: string, extra: object = {}) => ({
        timestamp: '2026-11-27T07:00:00Z',
        user,
        content: { chat: 'hi' },
        ...extra,
    })

    it('names a connected account by its Vizier user', () => {
        const metadata = { platform_display_name: 'Rin D.' }
        expect(shownSender(request('rin', { verified: true, metadata }))).toEqual({ name: 'rin', seed: 'rin', sender: 'connected' })
    })

    it('names an unconnected account as its platform does, and tells two apart', () => {
        const kai = shownSender(request('Unregistered User', { verified: false, metadata: { platform_display_name: 'Kai', platform_user_id: '2' } }))
        expect(kai).toEqual({ name: 'Kai', seed: '2', sender: 'unregistered' })
        const handle = shownSender(request('Unregistered User', { verified: false, metadata: { platform_username: 'sam', platform_user_id: '3' } }))
        expect(handle.name).toBe('sam')
        expect(handle.seed).not.toBe(kai.seed)
        expect(shownSender(request('Unregistered User', { verified: false })).name).toBe('Unregistered User')
    })

    it('shows an older message as it was saved', () => {
        expect(shownSender(request('@dani (DiscordId: 182)'))).toEqual({ name: '@dani', seed: '@dani (DiscordId: 182)' })
    })
})

describe('naming an account', () => {
    it('writes one "@", and the id when there is no handle', () => {
        expect(bareHandle(' @rin ')).toBe('rin')
        expect(bareHandle('')).toBeUndefined()
        expect(accountName({ platform_username: '@rin_dev', platform_user_id: '1' })).toBe('@rin_dev')
        expect(accountName({ platform_username: null, platform_user_id: '55120' })).toBe('account 55120')
    })
})
