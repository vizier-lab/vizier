import { CodeBlock, Prose } from '@vizier/design'
import type { ComponentProps } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import remarkGfm from 'remark-gfm'

type Props = { children: string; size?: ComponentProps<typeof Prose>['size'] }

const components: ComponentProps<typeof ReactMarkdown>['components'] = {
    a: ({ node: _n, ...p }) => <a {...p} target="_blank" rel="noreferrer noopener" />,
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
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[[rehypeHighlight, { detect: false }]]} components={components}>
                {children}
            </ReactMarkdown>
        </Prose>
    )
}
