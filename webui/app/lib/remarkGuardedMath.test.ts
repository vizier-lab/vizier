import { describe, expect, it } from 'vitest'
import { isMathSpan } from './remarkGuardedMath'

/**
 * Whether a span of the source is math, decided the way GitHub decides it: the opening dollar
 * must not be followed by whitespace, the closing dollar must not be preceded by whitespace, and
 * the closing dollar must not be followed by a digit.
 */
const math = (source: string, span: string) => {
    const start = source.indexOf(span)
    expect(start).toBeGreaterThan(-1)
    return isMathSpan(source, start, start + span.length)
}

describe('the dollars of an inline math span', () => {
    // The bug: a price in prose was read as a formula, and the words between the dollars were
    // drawn as math.
    it('are not math when the closing dollar follows a space', () => {
        expect(math('The price is $5 and $10 total', '$5 and $')).toBe(false)
        expect(math('Use $HOME and $PATH here', '$HOME and $')).toBe(false)
        expect(math('Earn $100, spend $200, save $300', '$100, spend $')).toBe(false)
    })

    it('are not math when the opening dollar follows a space', () => {
        expect(math('Between $ and $ there is text', '$ and $')).toBe(false)
    })

    // A closing dollar followed by a digit is a price, not the end of a formula.
    it('are not math when the closing dollar is followed by a digit', () => {
        expect(math('It costs between $100–$200 for the tier.', '$100–$')).toBe(false)
    })

    it('are math when they are written like math', () => {
        expect(math('a formula $e^{i\\pi} + 1 = 0$ here', '$e^{i\\pi} + 1 = 0$')).toBe(true)
        expect(math('Let $x$ and $y$ be integers', '$x$')).toBe(true)
        expect(math('Let $x$ and $y$ be integers', '$y$')).toBe(true)
    })

    // Two dollars are written on purpose, whatever sits between them.
    it('are math when there are two of them', () => {
        expect(math('inline $$x^2$$ math', '$$x^2$$')).toBe(true)
    })
})
