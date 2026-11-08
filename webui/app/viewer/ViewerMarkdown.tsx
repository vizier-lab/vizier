// Markdown of a shared message. react-markdown without `rehype-raw`: raw HTML in the text is text.
// A link follows the link rules (https, http, mailto; the host shown when the label hides it), any
// other scheme is plain text, and an image is never fetched: only its alt text is drawn.
import { CodeBlock, Prose } from '@vizier/design'
import { useMemo, type ComponentProps } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeKatex from 'rehype-katex'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import { SafeLink } from '../lib/generativeUi/registry/anchor'
import { highlightCode } from '../lib/highlight'
import { remarkBreaks } from '../lib/remarkBreaks'

const PLUGINS: NonNullable<ComponentProps<typeof ReactMarkdown>['remarkPlugins']> = [remarkGfm, remarkBreaks, remarkMath]
const REHYPE: NonNullable<ComponentProps<typeof ReactMarkdown>['rehypePlugins']> = [rehypeKatex]

function Fenced({ text, lang }: { text: string; lang: string | undefined }) {
    const tokens = useMemo(() => highlightCode(text, lang), [text, lang])
    return <CodeBlock code={text} tokens={tokens} language={lang} />
}

const components: ComponentProps<typeof ReactMarkdown>['components'] = {
    a: ({ node: _n, href, children }) => <SafeLink href={href ?? ''} label={children} />,
    img: ({ node: _n, alt }) => <span className="type-small text-faint">[image{alt ? `: ${alt}` : ''}]</span>,
    pre: ({ children }) => <>{children}</>,
    code: ({ className, children }) => {
        const text = String(children).replace(/\n$/, '')
        const lang = /language-([^\s]+)/.exec(className ?? '')?.[1]?.toLowerCase()
        if (!lang && !text.includes('\n')) return <code>{text}</code>
        return <Fenced text={text} lang={lang} />
    },
}

// The default URL transform drops `javascript:`; a link the rules refuse is text either way, and
// this keeps react-markdown from handing `SafeLink` a rewritten address.
const keepUrl = (url: string) => url

export function ViewerMarkdown({ children }: { children: string }) {
    return (
        <Prose size="reply">
            <ReactMarkdown remarkPlugins={PLUGINS} rehypePlugins={REHYPE} components={components} urlTransform={keepUrl}>
                {children}
            </ReactMarkdown>
        </Prose>
    )
}
