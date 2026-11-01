import { CodeBlock, Prose } from '@vizier/design'
import { useMemo, type ComponentProps } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeKatex from 'rehype-katex'
import { Link } from 'react-router'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import { highlightCode } from '../../lib/highlight'
import { remarkBreaks } from '../../lib/remarkBreaks'
import { Mermaid } from './Mermaid'

type Props = { children: string; size?: ComponentProps<typeof Prose>['size'] }

// Module level: a fresh plugin array on every render re-parses the whole document each time.
const PLUGINS: NonNullable<ComponentProps<typeof ReactMarkdown>['remarkPlugins']> = [remarkGfm, remarkBreaks, remarkMath]

// KaTeX is the one rehype pass. It rewrites math nodes only — `inlineMath` and `math`, which
// `remarkMath` produced — so a fence's text is still the source when `code` reads it below.
const REHYPE: NonNullable<ComponentProps<typeof ReactMarkdown>['rehypePlugins']> = [rehypeKatex]

/**
 * A fenced block: a diagram for mermaid, highlighted code for everything else. The tokens are
 * computed once per fence rather than per render, which is why this is a component of its own.
 */
function Fenced({ text, lang }: { text: string; lang: string | undefined }) {
    const tokens = useMemo(() => highlightCode(text, lang), [text, lang])
    if (lang === 'mermaid') return <Mermaid code={text} />
    return <CodeBlock code={text} tokens={tokens} language={lang} />
}

const components: ComponentProps<typeof ReactMarkdown>['components'] = {
    // A link into the app (a memory wiki link) stays in the tab; anything else opens a new one.
    a: ({ node: _n, href, ...p }) =>
        href?.startsWith('/') && !href.startsWith('//') ? <Link to={href} {...p} /> : <a href={href} {...p} target="_blank" rel="noreferrer noopener" />,
    // No rehype pass may rewrite a block: `code` reads its children as the source text, and a plugin
    // that highlights (rehype-highlight) replaces that text with elements, which draws as [object Object].
    pre: ({ children }) => <>{children}</>,
    code: ({ className, children }) => {
        const text = String(children).replace(/\n$/, '')
        const lang = /language-([^\s]+)/.exec(className ?? '')?.[1]?.toLowerCase()
        if (!lang && !text.includes('\n')) return <code>{text}</code>
        return <Fenced text={text} lang={lang} />
    },
}

/**
 * Markdown in a reply or document: react-markdown inside the library's Prose.
 *
 * A single newline is a line break everywhere, as it is in a chat: CommonMark calls that a soft break
 * and draws it as a space, so `remarkBreaks` turns it into a `<br/>`. One rule for every surface, so a
 * reply and the Core page holding the same text cannot disagree. Fenced blocks, inline code and tables
 * are untouched.
 *
 * Two things are drawn rather than printed: a fenced block (highlighted, wrapped, or a mermaid
 * diagram) and math (`$…$` and `$$…$$`, through KaTeX). A fence always wins: `$` inside one is
 * source, because the highlighter reads it as text and KaTeX never sees it.
 */
export function Markdown({ children, size = 'reply' }: Props) {
    return (
        <Prose size={size}>
            <ReactMarkdown remarkPlugins={PLUGINS} rehypePlugins={REHYPE} components={components}>
                {children}
            </ReactMarkdown>
        </Prose>
    )
}
