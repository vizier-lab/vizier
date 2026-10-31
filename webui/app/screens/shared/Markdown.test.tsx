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

describe('markdown line breaks', () => {
    // One rule everywhere: a newline is a line break, whether the text came from a chat or a document.
    it('draws a single newline as a line break', () => {
        expect(draw('one\ntwo')).toContain('<br/>')
    })

    it('draws a newline after inline markup', () => {
        expect(draw('**one**\ntwo')).toContain('<br/>')
    })

    it('breaks a continued line inside a list item', () => {
        expect(draw('- one\n  two')).toContain('<br/>')
    })

    it('draws a blank line as a paragraph, not as a break', () => {
        const html = draw('one\n\ntwo')
        expect(html).toContain('<p>one</p>')
        expect(html).toContain('<p>two</p>')
        expect(html).not.toContain('<br/>')
    })

    // The two ways to say "break here" must not stack into a double break.
    it('draws an explicit hard break once', () => {
        const brs = (markdown: string) => draw(markdown).match(/<br\/>/g)?.length ?? 0
        expect(brs('one  \ntwo')).toBe(1)
        expect(brs('one\\\ntwo')).toBe(1)
    })

    it('leaves a fenced block as source text', () => {
        const html = draw('```yaml\nagent:\n  name: oz\n```')
        expect(html).not.toContain('<br/>')
        expect(html).toContain('name: oz')
    })

    it('leaves an indented block as source text', () => {
        expect(draw('text:\n\n    one\n    two')).not.toContain('<br/>')
    })

    it('leaves a table row on one line', () => {
        const html = draw('| a | b |\n| - | - |\n| 1 | 2 |')
        expect(html).not.toContain('<br/>')
        expect(html).toContain('<td>1</td>')
    })

    it('leaves a heading on one line', () => {
        const html = draw('# one\ntwo')
        expect(html).not.toContain('<br/>')
        expect(html).toContain('<h1>one</h1>')
    })
})
