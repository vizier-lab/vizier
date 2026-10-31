import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { VizierAttachment } from '../../api/frames'

const mode = vi.hoisted(() => ({ desktop: false }))

vi.mock('../../api/transport', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../../api/transport')>()
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
            saveFile: vi.fn(),
        },
    }
})

import { attachmentUrl, ReplyMedia } from './ReplyMedia'

const picture: VizierAttachment = { filename: 'image.png', content: { local: '/api/v1/files/abc' } }
const remote: VizierAttachment = { filename: 'cat.png', content: { url: 'https://cdn.example/cat.png' } }

describe('attachments in the desktop app and the browser', () => {
    // The bug: the window fetched a bare /api/v1 path from its own origin and drew a broken image.
    it('addresses an upload through the asset scheme when desktop', () => {
        mode.desktop = true
        expect(attachmentUrl(picture)).toBe('vizier-asset://localhost/api/v1/files/abc')
    })

    it('addresses an upload through the API when in a browser', () => {
        mode.desktop = false
        expect(attachmentUrl(picture)).toBe('/api/v1/files/abc')
    })

    it('leaves a remote URL as it is, in either window', () => {
        mode.desktop = true
        expect(attachmentUrl(remote)).toBe('https://cdn.example/cat.png')
        mode.desktop = false
        expect(attachmentUrl(remote)).toBe('https://cdn.example/cat.png')
    })

    it('draws the picture at the address the window can load', () => {
        mode.desktop = true
        const html = renderToString(<ReplyMedia attachments={[picture]} />)
        expect(html).toContain('src="vizier-asset://localhost/api/v1/files/abc"')
    })
})
