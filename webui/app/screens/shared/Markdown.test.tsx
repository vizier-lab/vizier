import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { Markdown } from './Markdown'

/** A reply drawn as the chat draws it. */
const draw = (markdown: string) => renderToString(<Markdown>{markdown}</Markdown>)

describe('markdown code fences', () => {
    // The reply in the bug report: the plain text was right, the drawn block was not.
    it('draws a yaml fence as its source text', () => {
        const html = draw('```yaml\nagent:\n  name: oz\n  tools: [read_file]\n```')
        expect(html).toContain('agent:')
        expect(html).toContain('name: oz')
        expect(html).toContain('tools: [read_file]')
        expect(html).not.toContain('[object Object]')
    })

    it('draws a json fence as its source text', () => {
        const html = draw('```json\n{ "name": "oz", "tools": ["read_file"] }\n```')
        expect(html).toContain('&quot;name&quot;: &quot;oz&quot;')
        expect(html).not.toContain('[object Object]')
    })

    it('names the language on the block', () => {
        expect(draw('```yaml\nkey: value\n```')).toContain('yaml')
    })

    it('draws a fence with no language', () => {
        const html = draw('```\nplain text\nsecond line\n```')
        expect(html).toContain('plain text')
        expect(html).toContain('second line')
    })

    it('draws inline code', () => {
        expect(draw('call `read_file` first')).toContain('<code>read_file</code>')
    })
})
