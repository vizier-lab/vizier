import { CodeBlock, Prose } from '@vizier/design'
import type { ComponentProps } from 'react'
import ReactMarkdown from 'react-markdown'
import { Link } from 'react-router'
import remarkGfm from 'remark-gfm'

type Props = { children: string; size?: ComponentProps<typeof Prose>['size'] }

const components: ComponentProps<typeof ReactMarkdown>['components'] = {
    // A link into the app (a memory wiki link) stays in the tab; anything else opens a new one.
    a: ({ node: _n, href, ...p }) =>
        href?.startsWith('/') && !href.startsWith('//') ? <Link to={href} {...p} /> : <a href={href} {...p} target="_blank" rel="noreferrer noopener" />,
    // No rehype pass may rewrite a block: `code` reads its children as the source text, and a plugin
    // that highlights (rehype-highlight) replaces that text with elements, which draws as [object Object].
    pre: ({ children }) => <>{children}</>,
    code: ({ className, children }) => {
        const text = String(children).replace(/\n$/, '')
        const lang = /language-(\w+)/.exec(className ?? '')?.[1]
        if (!lang && !text.includes('\n')) return <code>{text}</code>
        return <CodeBlock code={text} language={lang} />
    },
}

/** Markdown in a reply or document: react-markdown inside the library's Prose. */
export function Markdown({ children, size = 'reply' }: Props) {
    return (
        <Prose size={size}>
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
                {children}
            </ReactMarkdown>
        </Prose>
    )
}
