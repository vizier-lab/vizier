import { renderToString } from 'react-dom/server'
import { MemoryRouter } from 'react-router'
import { describe, expect, it } from 'vitest'
import { EntityBroken } from '../../lib/entityLinks'
import { Markdown } from './Markdown'

/** A reply drawn as the chat draws it. */
const draw = (markdown: string) => renderToString(<Markdown>{markdown}</Markdown>)

/** The same, inside a router and an entity-link scope, as the chat and Memory draw it. */
const drawRouted = (markdown: string, broken?: ReadonlySet<string>) =>
    renderToString(
        <MemoryRouter>
            <EntityBroken.Provider value={broken ?? null}>
                <Markdown>{markdown}</Markdown>
            </EntityBroken.Provider>
        </MemoryRouter>
    )

/**
 * What the block actually reads, whatever it is coloured as: tags, React's hydration comments
 * and HTML entities taken out. A highlighted block is many elements where a plain one was one
 * string, so the source is asserted here rather than against the markup.
 */
const shown = (html: string) =>
    html
        // From the code itself: the language label and the copy button above it are chrome.
        .slice(html.indexOf('<pre'))
        .replace(/<!--.*?-->/g, '')
        .replace(/<[^>]*>/g, '')
        .replace(/&quot;/g, '"')
        .replace(/&#x27;/g, "'")
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&amp;/g, '&')

/**
 * What the prose reads, without the markup around it. React's hydration comments separate two
 * text nodes, so they come out before the tags do.
 */
const text = (html: string) =>
    html
        .replace(/<!--.*?-->/g, '')
        .replace(/<[^>]*>/g, '')
        .replace(/&quot;/g, '"')
        .replace(/&#x27;/g, "'")
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&amp;/g, '&')

describe('markdown code fences', () => {
    // The reply in the bug report: the plain text was right, the drawn block was not.
    it('draws a yaml fence as its source text', () => {
        const html = draw('```yaml\nagent:\n  name: oz\n  tools: [read_file]\n```')
        expect(shown(html)).toBe('agent:  name: oz  tools: [read_file]')
        expect(html).not.toContain('[object Object]')
    })

    it('draws a json fence as its source text', () => {
        const html = draw('```json\n{ "name": "oz", "tools": ["read_file"] }\n```')
        expect(shown(html)).toBe('{ "name": "oz", "tools": ["read_file"] }')
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
        expect(shown(html)).toBe('agent:  name: oz')
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

describe('markdown highlighting', () => {
    // The block is drawn from tokens now, so each tone is a class of its own; the text is still
    // the source, which is what the tests above keep locking.
    it('colours a fence by its language', () => {
        const html = draw('```python\nfor x in hits:\n    print("db", 8)\n```')
        expect(html).toContain('text-code-keyword')
        expect(html).toContain('text-code-string')
        expect(shown(html)).toBe('for x in hits:    print("db", 8)')
        expect(html).not.toContain('[object Object]')
    })

    it('draws a fence in a language it does not highlight as its source', () => {
        const html = draw('```brainfuck\n+++.\n```')
        expect(html).toContain('+++.')
        expect(html).not.toContain('[object Object]')
    })
})

describe('markdown math', () => {
    it('draws inline math', () => {
        const html = draw('a formula $e^{i\\pi} + 1 = 0$ here')
        expect(html).toContain('class="katex"')
        // KaTeX draws the exponent as markup, not as the source it was given.
        expect(html).toContain('mord')
    })

    it('draws display math on its own line', () => {
        expect(draw('$$\n\\int_0^1 x^2\\,dx\n$$')).toContain('katex-display')
    })

    // A fence is source: the highlighter reads it as text and KaTeX never sees it. A price in
    // prose is the price of `$…$`; a fence is where it must not happen.
    it('leaves a dollar inside a fence alone', () => {
        const html = draw('```python\nprice = "$5"\n```')
        expect(html).not.toContain('class="katex"')
        expect(html).toContain('$5')
    })

    // The bug: a price in prose was read as a formula, and the words between the dollars were
    // drawn as math.
    it('reads a price in prose as prose', () => {
        const html = draw('The price is $5 and $10 total')
        expect(html).not.toContain('class="katex"')
        expect(text(html)).toBe('The price is $5 and $10 total')
    })

    it('reads shell variables in prose as prose', () => {
        const html = draw('Use $HOME and $PATH here')
        expect(html).not.toContain('class="katex"')
        expect(text(html)).toBe('Use $HOME and $PATH here')
    })

    // A formula is still a formula: the dollars around it are written like math.
    it('still reads a formula as math', () => {
        expect(draw('Let $x$ and $y$ be integers')).toContain('class="katex"')
    })
})

describe('markdown diagrams', () => {
    // Drawn on the client, so a render without effects shows the source — which is also the
    // fallback when mermaid cannot parse it.
    it('shows a mermaid fence as its source until it is drawn', () => {
        const html = draw('```mermaid\ngraph TD\n  a --> b\n```')
        expect(html).toContain('graph TD')
        expect(html).toContain('a --&gt; b')
        expect(html).toContain('mermaid')
    })
})

describe('entity links in markdown (spec 066)', () => {
    it('renders a live entity link as an internal link', () => {
        expect(drawRouted('[report](/oz/artifacts?artifact=report)')).toContain('href="/oz/artifacts?artifact=report"')
    })

    it('renders a resolved-missing entity link broken, going nowhere', () => {
        const html = drawRouted('[gone](/oz/artifacts?artifact=gone)', new Set(['artifact:gone']))
        expect(html).not.toContain('href=')
        expect(html).toContain('Not found')
        expect(html).toContain('line-through')
    })

    it('renders entity links optimistically live before a scope resolves them', () => {
        expect(drawRouted('[gone](/oz/artifacts?artifact=gone)')).toContain('href="/oz/artifacts?artifact=gone"')
    })

    it('leaves non-entity internal links (a memory link) alone', () => {
        const html = drawRouted('[memory](/oz/memory/research/notes)', new Set(['artifact:gone']))
        expect(html).toContain('href="/oz/memory/research/notes"')
    })

    it('links `[[kind:name]]` text for the agent it is given, and leaves it as text without one', () => {
        const md = 'Ran [[task:weekly-report|the weekly task]].'
        const linked = renderToString(
            <MemoryRouter>
                <Markdown entities="oz">{md}</Markdown>
            </MemoryRouter>
        )
        expect(linked).toContain('href="/oz/tasks/weekly-report"')
        expect(linked).toContain('the weekly task')
        const plain = renderToString(
            <MemoryRouter>
                <Markdown>{md}</Markdown>
            </MemoryRouter>
        )
        expect(plain).not.toContain('href=')
    })
})
