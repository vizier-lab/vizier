import { describe, expect, it } from 'vitest'
import { liveVoiceMissingFrom } from './voice'

// FR-004 (spec 055): the field names which half of a live voice session is missing. It picks a
// **sentence**, never readiness — `live_voice: bool` is what says whether the row may start a call.
describe('liveVoiceMissingFrom', () => {
    it('passes the two halves through as the server names them', () => {
        expect(liveVoiceMissingFrom({ live_voice_missing: 'recognizer' })).toBe('recognizer')
        expect(liveVoiceMissingFrom({ live_voice_missing: 'voice' })).toBe('voice')
    })

    it('reads null as "nothing to name"', () => {
        expect(liveVoiceMissingFrom({ live_voice_missing: null })).toBeNull()
    })

    it('reads an absent field the same as null — an older server', () => {
        expect(liveVoiceMissingFrom({})).toBeNull()
    })

    it('reads a null detail as nothing to name', () => {
        expect(liveVoiceMissingFrom(null)).toBeNull()
    })

    it('reads an undefined detail as nothing to name', () => {
        expect(liveVoiceMissingFrom(undefined)).toBeNull()
    })

    it('does not pass an unknown word through — a newer server must not pick a missing sentence', () => {
        expect(liveVoiceMissingFrom({ live_voice_missing: 'future-value' as any })).toBeNull()
    })
})
