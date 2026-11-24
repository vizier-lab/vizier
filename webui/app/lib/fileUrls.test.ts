import { describe, expect, it, vi } from 'vitest'

const mode = vi.hoisted(() => ({ desktop: false }))

// The two real transports differ only in how they address a server path, so the mock tells them
// apart the same way: the asset scheme in the desktop window, the API's own origin in a browser.
vi.mock('../api/transport', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../api/transport')>()
    const at = (path: string) => {
        const relative = `${actual.API_BASE}${actual.relativeToApi(path)}`
        return mode.desktop ? `vizier-asset://localhost${relative}` : relative
    }
    return {
        ...actual,
        transport: {
            get authMode() {
                return mode.desktop ? 'desktop' : 'http'
            },
            assetUrl: at,
        },
    }
})

import { assetPath, assetSrc } from './fileUrls'

describe('a server file as an element can load it', () => {
    it('uses the asset scheme in the desktop app', () => {
        mode.desktop = true
        expect(assetPath('/api/v1/files/abc')).toBe('vizier-asset://localhost/api/v1/files/abc')
        expect(assetPath('/files/abc')).toBe('vizier-asset://localhost/api/v1/files/abc')
    })

    it('uses the API origin in a browser', () => {
        mode.desktop = false
        expect(assetPath('/api/v1/files/abc')).toBe('/api/v1/files/abc')
        expect(assetPath('/files/abc')).toBe('/api/v1/files/abc')
    })

    it('passes a missing path through as undefined', () => {
        expect(assetSrc(null)).toBeUndefined()
        expect(assetSrc('')).toBeUndefined()
        expect(assetSrc('/api/v1/files/abc')).toBe('/api/v1/files/abc')
    })
})
