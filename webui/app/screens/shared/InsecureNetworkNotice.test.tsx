import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { InsecureNetworkNotice } from './InsecureNetworkNotice'

/** What the notice says, without the markup around it. */
const said = (html: string) =>
    html
        .replace(/<[^>]*>/g, '')
        .replace(/&#x27;/g, "'")
        .replace(/&quot;/g, '"')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')

describe('the LAN address notice', () => {
    it('says that the install is loopback-only, and why', () => {
        const text = said(renderToString(<InsecureNetworkNotice />))
        expect(text).toContain('Install is loopback-only')
        expect(text).toContain('the browser treats the connection as insecure')
        expect(text).toContain('http://127.0.0.1:')
    })
})
