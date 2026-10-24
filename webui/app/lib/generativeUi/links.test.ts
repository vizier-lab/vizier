import { describe, expect, it } from 'vitest'
import { hostToDisclose, markdownLinks, parseHref } from './links'

describe('parseHref', () => {
    it.each(['https://example.com', 'http://example.com/a?b=1#c', 'mailto:a@example.com', 'HTTPS://EXAMPLE.COM', 'https://example.com:8443/x'])('allows %s', (href) => {
        expect(parseHref(href)).not.toBeNull()
    })

    it.each([
        'javascript:alert(1)',
        'JaVaScRiPt:alert(1)',
        'java\tscript:alert(1)',
        'java\nscript:alert(1)',
        ' javascript:alert(1)',
        '\u0001javascript:alert(1)',
        'data:text/html,<script>alert(1)</script>',
        'file:///etc/passwd',
        'blob:https://example.com/uuid',
        '//evil.test/path',
        'ftp://example.com',
        'vbscript:x',
        'https://user:pass@example.com',
        'https://paypal.com@evil.test',
        'https://',
        'mailto:',
        'mailto:nobody',
        'example.com',
        '',
        'https://exa mple.com',
    ])('refuses %j', (href) => {
        expect(parseHref(href)).toBeNull()
    })

    it('refuses non-strings and very long addresses', () => {
        expect(parseHref(undefined)).toBeNull()
        expect(parseHref(42)).toBeNull()
        expect(parseHref(`https://example.com/${'a'.repeat(2100)}`)).toBeNull()
    })

    it('finds the real host', () => {
        expect(parseHref('https://evil.test/login')?.host).toBe('evil.test')
        expect(parseHref('mailto:ada@example.org')?.host).toBe('example.org')
    })
})

describe('hostToDisclose', () => {
    const link = parseHref('https://evil.test/login')!

    it('shows the host beside a label that hides it', () => {
        expect(hostToDisclose('paypal.com', link)).toBe('evil.test')
        expect(hostToDisclose('Docs', link)).toBe('evil.test')
    })

    it('shows nothing when the label already says it', () => {
        expect(hostToDisclose('evil.test', link)).toBeNull()
        expect(hostToDisclose('https://evil.test/login', link)).toBeNull()
        expect(hostToDisclose('www.example.com', parseHref('https://www.example.com')!)).toBeNull()
        expect(hostToDisclose('example.com', parseHref('https://www.example.com')!)).toBeNull()
    })
})

describe('markdownLinks', () => {
    it('lists the targets of markdown links', () => {
        expect(markdownLinks('see [a](https://x.test) and [b]( javascript:alert(1) ) ok')).toEqual(['https://x.test', 'javascript:alert(1'])
    })
})
