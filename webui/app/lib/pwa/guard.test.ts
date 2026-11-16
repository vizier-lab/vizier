import { describe, expect, it } from 'vitest'
import { isInsecureNetwork, isSecureOrigin, shouldRegister } from './guard'

describe('the service worker guard', () => {
    it('registers on a loopback address', () => {
        expect(shouldRegister({ hostname: '127.0.0.1', protocol: 'http:', desktop: false })).toBe(true)
        expect(shouldRegister({ hostname: 'localhost', protocol: 'http:', desktop: false })).toBe(true)
    })

    it('registers over HTTPS', () => {
        expect(shouldRegister({ hostname: 'vizier.rs', protocol: 'https:', desktop: false })).toBe(true)
    })

    it('does not register on a LAN address over plain HTTP', () => {
        expect(shouldRegister({ hostname: '192.168.1.20', protocol: 'http:', desktop: false })).toBe(false)
        expect(isInsecureNetwork({ hostname: '192.168.1.20', protocol: 'http:' })).toBe(true)
    })

    it('does not register in the desktop app', () => {
        expect(shouldRegister({ hostname: 'localhost', protocol: 'http:', desktop: true })).toBe(false)
        expect(shouldRegister({ hostname: 'app', protocol: 'tauri:', desktop: true })).toBe(false)
    })

    it('calls a secure origin only what the browser calls one', () => {
        expect(isSecureOrigin({ hostname: '127.0.0.1', protocol: 'http:', desktop: false })).toBe(true)
        expect(isSecureOrigin({ hostname: 'localhost', protocol: 'http:', desktop: false })).toBe(true)
        expect(isSecureOrigin({ hostname: '[::1]', protocol: 'http:', desktop: false })).toBe(true)
        expect(isSecureOrigin({ hostname: 'vizier.rs', protocol: 'https:', desktop: false })).toBe(true)
        expect(isSecureOrigin({ hostname: '192.168.1.20', protocol: 'http:', desktop: false })).toBe(false)
        expect(isSecureOrigin({ hostname: 'vizier.rs', protocol: 'http:', desktop: false })).toBe(false)
    })

    it('knows a LAN address from a secure one', () => {
        expect(isInsecureNetwork({ hostname: '192.168.1.20', protocol: 'http:', desktop: false })).toBe(true)
        expect(isInsecureNetwork({ hostname: 'vizier.rs', protocol: 'http:', desktop: false })).toBe(true)
        expect(isInsecureNetwork({ hostname: '127.0.0.1', protocol: 'http:', desktop: false })).toBe(false)
        expect(isInsecureNetwork({ hostname: '192.168.1.20', protocol: 'https:', desktop: false })).toBe(false)
    })
})
