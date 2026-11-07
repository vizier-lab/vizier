import { afterEach, describe, expect, it } from 'vitest'
import { setInstanceZone, wallClockString, zoned } from './timezone'

// Read through fields, so these hold whichever zone the runner is in.
const fields = (d: Date) => [d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds()]

afterEach(() => setInstanceZone(null))

describe('zoned', () => {
    it('is the real date before a zone is loaded', () => {
        const at = new Date('2026-11-07T02:00:00Z')
        expect(zoned(at).getTime()).toBe(at.getTime())
    })

    it('shows the wall clock of a named zone', () => {
        setInstanceZone({ name: 'Pacific/Kiritimati', offset: '+14:00' })
        expect(fields(zoned('2026-11-07T02:03:04Z'))).toEqual([2026, 11, 7, 16, 3, 4])
        setInstanceZone({ name: 'Pacific/Pago_Pago', offset: '-11:00' })
        expect(fields(zoned('2026-11-07T02:03:04Z'))).toEqual([2026, 11, 6, 15, 3, 4])
    })

    it('keeps the same instant whatever offset the text carries', () => {
        setInstanceZone({ name: 'Asia/Jakarta', offset: '+07:00' })
        expect(fields(zoned('2026-11-07T09:00:00+07:00'))).toEqual(fields(zoned('2026-11-07T02:00:00Z')))
    })

    it('follows daylight saving either side of a change', () => {
        setInstanceZone({ name: 'America/New_York', offset: '-05:00' })
        expect(fields(zoned('2030-03-10T06:59:00Z'))).toEqual([2030, 3, 10, 1, 59, 0])
        expect(fields(zoned('2030-03-10T07:00:00Z'))).toEqual([2030, 3, 10, 3, 0, 0])
    })

    it('renders midnight as 00, not 24', () => {
        setInstanceZone({ name: 'Asia/Jakarta', offset: '+07:00' })
        expect(fields(zoned('2026-11-06T17:00:00Z'))).toEqual([2026, 11, 7, 0, 0, 0])
    })

    it('uses the fixed offset when the zone has no name', () => {
        setInstanceZone({ name: null, offset: '+05:30' })
        expect(fields(zoned('2026-11-07T02:00:00Z'))).toEqual([2026, 11, 7, 7, 30, 0])
        setInstanceZone({ name: null, offset: '-03:00' })
        expect(fields(zoned('2026-11-07T02:00:00Z'))).toEqual([2026, 11, 6, 23, 0, 0])
    })

    it('falls back to the offset for a name the browser does not know', () => {
        setInstanceZone({ name: 'Not/AZone', offset: '+02:00' })
        expect(fields(zoned('2026-11-07T02:00:00Z'))).toEqual([2026, 11, 7, 4, 0, 0])
    })
})

describe('wallClockString', () => {
    it('writes the browser-local fields with no zone', () => {
        expect(wallClockString(new Date(2026, 10, 7, 9, 5, 0))).toBe('2026-11-07T09:05:00')
    })

    it('round-trips what zoned shows', () => {
        setInstanceZone({ name: 'Asia/Jakarta', offset: '+07:00' })
        expect(wallClockString(zoned('2026-11-07T02:00:00Z'))).toBe('2026-11-07T09:00:00')
    })
})
