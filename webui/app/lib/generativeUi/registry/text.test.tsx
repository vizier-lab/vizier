import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { MarkdownText } from './text'

/** Interface text, drawn as the reply is. */
const draw = (markdown: string) => renderToString(<MarkdownText>{markdown}</MarkdownText>)

describe('interface text', () => {
    // A model writing a line per line means the lines, the same as in a reply.
    it('draws a single newline as a line break', () => {
        expect(draw('one\ntwo')).toContain('<br/>')
    })

    it('keeps the newlines of a fenced block', () => {
        const html = draw('```\none\ntwo\n```')
        expect(html).not.toContain('<br/>')
    })
})
