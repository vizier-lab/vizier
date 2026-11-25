import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ViewerMarkdown } from './ViewerMarkdown'

describe('entity links in a shared conversation (spec 066)', () => {
    it('renders an entity link as inert text — no navigation to a live agent page', () => {
        const html = renderToString(<ViewerMarkdown>{'See [report](/oz/artifacts?artifact=report).'}</ViewerMarkdown>)
        expect(html).not.toContain('<a')
        expect(html).toContain('report')
    })

    it('keeps the ordinary link rules for a real address', () => {
        const html = renderToString(<ViewerMarkdown>{'[site](https://example.com)'}</ViewerMarkdown>)
        expect(html).toContain('<a')
    })
})
