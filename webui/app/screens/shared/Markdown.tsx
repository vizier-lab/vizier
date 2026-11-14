import { CodeBlock, MentionChip, Prose } from '@vizier/design'
import { createContext, useContext, useMemo, type ComponentProps } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeKatex from 'rehype-katex'
import { Link } from 'react-router'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import { highlightCode } from '../../lib/highlight'
import { remarkBreaks } from '../../lib/remarkBreaks'
import { Mermaid } from './Mermaid'

type Props = {
    children: string
    size?: ComponentProps<typeof Prose>['size']
    tone?: ComponentProps<typeof Prose>['tone']
}

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

/**
 * Who a Workspace message may name. A mention drawn as inline code (`mentionsAsCode`) whose handle is
 * in `handles` shows as a chip instead, the signed-in person's in the colour that means "you".
 * Outside a Workspace there is no value, and it stays inline code.
 */
export const MentionHandles = createContext<{ handles: ReadonlySet<string>; me: string } | null>(null)

function InlineCode({ text }: { text: string }) {
    const mentions = useContext(MentionHandles)
    const handle = text.startsWith('@') ? text.slice(1) : null
    if (mentions && handle && mentions.handles.has(handle)) {
        return <MentionChip handle={handle} tone={handle === mentions.me ? 'you' : 'resolved'} />
    }
    return <code>{text}</code>
}

const components: ComponentProps<typeof ReactMarkdown>['components'] = {
    // A link into the app (a memory wiki link) stays in the tab; anything else opens a new one.
    a: ({ node: _n, href, ...p }) =>
        href?.startsWith('/') && !href.startsWith('//') ? <Link to={href} {...p} /> : <a href={href} {...p} target="_blank" rel="noreferrer noopener" />,
    // An image from another site is sent no referrer: the app's origin is nobody's business, and
    // hosts that refuse foreign referrers (hotlink protection) still serve a request without one.
    img: ({ node: _n, ...p }) => <img {...p} referrerPolicy="no-referrer" loading="lazy" />,
    // No rehype pass may rewrite a block: `code` reads its children as the source text, and a plugin
    // that highlights (rehype-highlight) replaces that text with elements, which draws as [object Object].
    pre: ({ children }) => <>{children}</>,
    code: ({ className, children }) => {
        const text = String(children).replace(/\n$/, '')
        const lang = /language-([^\s]+)/.exec(className ?? '')?.[1]?.toLowerCase()
        if (!lang && !text.includes('\n')) return <InlineCode text={text} />
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
export function Markdown({ children, size = 'reply', tone }: Props) {
    return (
        <Prose size={size} tone={tone}>
            <ReactMarkdown remarkPlugins={PLUGINS} rehypePlugins={REHYPE} components={components}>
                {children}
            </ReactMarkdown>
        </Prose>
    )
}
