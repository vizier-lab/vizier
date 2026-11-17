import { describe, expect, it } from 'vitest'
import { elementStyles, formatValue, initialValue } from './values'

describe('element values', () => {
    it('formats numbers as the page asked', () => {
        expect(formatValue(0.421, 'percent')).toBe('42.1%')
        expect(formatValue(1536, 'bytes')).toBe('1.5 KB')
        expect(formatValue(3725, 'duration')).toBe('1h 2m')
        expect(formatValue(75, 'duration')).toBe('1m 15s')
        expect(formatValue('n/a', 'percent')).toBe('n/a')
        expect(formatValue(undefined, undefined)).toBe('')
    })

    it('starts a field from its value attribute, by kind', () => {
        expect(initialValue('checkbox', 'true', undefined)).toBe(true)
        expect(initialValue('switch', undefined, undefined)).toBe(false)
        expect(initialValue('number', '12', undefined)).toBe(12)
        expect(initialValue('number', 'twelve', undefined)).toBe('')
        expect(initialValue('slider', undefined, 5)).toBe(5)
        expect(initialValue('text', undefined, undefined)).toBe('')
    })

    it('drops font faces from the elements stylesheet', () => {
        expect(elementStyles('a{b:c}@font-face{font-family:x;src:url(y.woff2)}d{e:f}')).toBe('a{b:c}d{e:f}')
    })
})
