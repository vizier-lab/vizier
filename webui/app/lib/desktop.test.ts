import { describe, expect, it } from 'vitest'
import { bootstrapView, errorCode, failureMessage, lockHolder, ownerCandidates, pollDelay, shouldNotifyOs, stoppedSubsystems, validPort } from './desktop'

describe('desktop helpers', () => {
    it('shows the app only when ready or degraded', () => {
        expect(bootstrapView(null)).toBe('starting')
        expect(bootstrapView({ state: 'ready' })).toBe('app')
        expect(bootstrapView({ state: 'degraded' })).toBe('app')
        expect(bootstrapView({ state: 'locked' })).toBe('locked')
        expect(bootstrapView({ state: 'needs_owner' })).toBe('needs_owner')
    })
    it('reads state detail defensively', () => {
        expect(stoppedSubsystems({ state: 'degraded', detail: { stopped: ['agents', 3] } })).toEqual(['agents'])
        expect(stoppedSubsystems({ state: 'ready' })).toEqual([])
        expect(ownerCandidates({ state: 'choose_owner', detail: { candidates: [{ user_id: 'a', username: 'A' }, 5] } })).toHaveLength(1)
        expect(lockHolder({ state: 'locked', detail: { holder: 'pid 4' } })).toBe('pid 4')
        expect(failureMessage({ state: 'failed', detail: { error: 'x' } })).toBe('x')
        expect(failureMessage({ state: 'failed' })).toBeUndefined()
    })
    it('error codes', () => {
        expect(errorCode({ code: 'password_required' })).toBe('password_required')
        expect(errorCode(new Error('x'))).toBeUndefined()
    })
    it('notifies the OS when hidden or unfocused', () => {
        expect(shouldNotifyOs('hidden', true)).toBe(true)
        expect(shouldNotifyOs('visible', false)).toBe(true)
        expect(shouldNotifyOs('visible', true)).toBe(false)
    })
    it('poll delay', () => {
        expect(pollDelay(false, 'hidden')).toBeNull()
        expect(pollDelay(true, 'hidden')).toBe(60_000)
        expect(pollDelay(true, 'visible')).toBe(15_000)
    })
    it('ports', () => {
        expect(validPort(9999)).toBe(true)
        expect(validPort(0)).toBe(false)
        expect(validPort(70000)).toBe(false)
    })
})
