import { Notice, TextLink } from '@vizier/design'
import type { NoticeProps } from '@vizier/design'

type Props = {
    kind: string
    message: string
    agentName: string
    onRetry?: () => void
    configHref: string
    providersHref: string
    healthHref: string
}

/** Each error kind as the Chat states sheet draws it: its tone, its words, its actions (FR-008). */
export function ErrorNotice({ kind, message, agentName, onRetry, configHref, providersHref, healthHref }: Props) {
    let tone: NoticeProps['tone'] = 'failed'
    let title = `${agentName} couldn't answer`
    let actions: React.ReactNode = onRetry ? <TextLink onClick={onRetry}>Retry</TextLink> : undefined

    switch (kind) {
        case 'rate_limited':
            tone = 'retrying'
            title = 'The provider is rate limiting this agent'
            actions = (
                <>
                    {onRetry && <TextLink onClick={onRetry}>Retry</TextLink>}
                    <TextLink href={configHref}>Switch model</TextLink>
                </>
            )
            break
        case 'provider_unavailable':
            title = "The provider isn't answering"
            actions = (
                <>
                    {onRetry && <TextLink onClick={onRetry}>Retry</TextLink>}
                    <TextLink href={configHref}>Switch model</TextLink>
                    <TextLink href={providersHref} arrow="forward">Open Providers</TextLink>
                </>
            )
            break
        case 'agent_unavailable':
            title = `${agentName} is not running`
            actions = (
                <>
                    {onRetry && <TextLink onClick={onRetry}>Retry</TextLink>}
                    <TextLink href={healthHref} arrow="forward">Open agent health</TextLink>
                </>
            )
            break
        case 'tool_timeout':
            title = 'A tool took too long'
            break
        case 'prompt_timeout':
            title = `${agentName} took too long to answer`
            break
    }

    return (
        <Notice tone={tone} title={title} marker="dot" actions={actions}>
            {message}
        </Notice>
    )
}
