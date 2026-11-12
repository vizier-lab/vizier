import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { OfflineBanner } from './OfflineBanner'

/** What the banner says, without the markup around it. */
const said = (html: string) =>
    html
        .replace(/<[^>]*>/g, '')
        .replace(/&#x27;/g, "'")
        .replace(/&quot;/g, '"')
        .replace(/&amp;/g, '&')

describe('the offline banner', () => {
    it('says what is unavailable, not that you are offline', () => {
        const text = said(renderToString(<OfflineBanner onRetry={() => {}} />))
        expect(text).toContain("Vizier can't reach the engine")
        expect(text).toContain('Replies and new conversations need a connection')
        expect(text).toContain('What is already open stays readable')
    })

    it('offers a way to try again', () => {
        const text = said(renderToString(<OfflineBanner onRetry={() => {}} />))
        expect(text).toContain('Try again')
    })
})
