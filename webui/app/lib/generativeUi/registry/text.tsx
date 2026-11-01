import { Badge, Callout as Hint, CodeBlock, Notice } from '@vizier/design'
import { useMemo } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { highlightCode } from '../../highlight'
import { remarkBreaks } from '../../remarkBreaks'
import { Prose } from '@vizier/design'
import type { ComponentRenderProps } from '@json-render/react'
import { SafeLink } from './anchor'

type P = Record<string, unknown>

const BADGE = { default: 'neutral', accent: 'rank', ok: 'live', warn: 'retrying', error: 'failed' } as const
const NOTICE = { default: 'info', accent: 'info', ok: 'info', warn: 'retrying', error: 'failed' } as const
const TEXT_TONE = { normal: 'text-fg', muted: 'text-muted', ok: 'text-live-text', warn: 'text-retrying', error: 'text-failed-text' } as const
const TEXT_SIZE = { sm: 'type-small', md: 'type-ui-para', lg: 'type-body-para' } as const

export function HeadingC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const level = typeof p.level === 'number' ? Math.min(4, Math.max(1, Math.round(p.level))) : 2
    const H = `h${level}` as 'h1' | 'h2' | 'h3' | 'h4'
    const style = level === 1 ? 'type-title' : level === 2 ? 'type-heading' : 'type-body-strong'
    return <H className={`${style} text-fg`}>{String(p.text ?? '')}</H>
}

const ALLOWED = ['p', 'strong', 'em', 'code', 'ul', 'ol', 'li', 'a', 'br', 'del']

/** The markdown subset an interface may use. A link is drawn by `renderLink` (the link rules), or as plain text. */
export function MarkdownText({ children, renderLink }: { children: string; renderLink?: (href: string, label: React.ReactNode) => React.ReactNode }) {
    return (
        <Prose size="reply">
            <ReactMarkdown
                remarkPlugins={[remarkGfm, remarkBreaks]}
                allowedElements={ALLOWED}
                unwrapDisallowed
                components={{ a: ({ node: _n, href, children: label }) => (renderLink && href ? renderLink(href, label) : <span>{label}</span>) }}
            >
                {children}
            </ReactMarkdown>
        </Prose>
    )
}

export function TextC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const tone = TEXT_TONE[(p.tone as keyof typeof TEXT_TONE) ?? 'normal'] ?? TEXT_TONE.normal
    const size = TEXT_SIZE[(p.size as keyof typeof TEXT_SIZE) ?? 'md'] ?? TEXT_SIZE.md
    return (
        <div className={`${tone} ${size}`}>
            <MarkdownText renderLink={(href, label) => <SafeLink href={href} label={label} />}>{String(p.text ?? '')}</MarkdownText>
        </div>
    )
}

export function BadgeC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    return <Badge tone={BADGE[(p.tone as keyof typeof BADGE) ?? 'default'] ?? 'neutral'}>{String(p.text ?? '')}</Badge>
}

export function CalloutC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const tone = (p.tone as keyof typeof NOTICE) ?? 'default'
    const title = typeof p.title === 'string' ? p.title : undefined
    // The quiet dashed hint is the default; a warning or an error uses the notice look.
    if (tone === 'warn' || tone === 'error') return <Notice tone={NOTICE[tone]} title={title}>{String(p.text ?? '')}</Notice>
    return <Hint title={title}>{String(p.text ?? '')}</Hint>
}

export function CodeC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const code = String(p.text ?? '')
    const lang = typeof p.lang === 'string' ? p.lang : undefined
    const tokens = useMemo(() => highlightCode(code, lang), [code, lang])
    return <CodeBlock code={code} tokens={tokens} language={lang} />
}
