import { Icon, PageHeader, type PageHeaderProps } from '@vizier/design'
import type { ReactNode } from 'react'
import { useMobile } from '../../lib/useMobile'
import { Link } from 'react-router'
import { NotificationsBell } from './NotificationsBell'
import { FadeRule } from './SideCard'

type Props = Omit<PageHeaderProps, 'children'> & {
    children?: ReactNode
    /**
     * `full` (the default) uses the whole width, as Chat does; `narrow` holds a form to a readable
     * measure, still starting on the same left edge. (`wide` and `workspace` are `full`.)
     */
    width?: 'narrow' | 'wide' | 'workspace' | 'full'
    /** The body is panes that fill the page and scroll themselves (Chat, Memory, Core): no gutter, no page scroll. */
    fill?: boolean
    /** A sub-page (an editor, one item) goes back to its parent page. */
    back?: { href: string; label: string; /** Navigation state to hand on, e.g. the way back past it. */ state?: unknown }
}

/**
 * The standard page: a header band the height of the agent switcher (and of the chat's), over a
 * body that runs edge to edge like the chat's. Every page shares the chat's 24px gutter, so the
 * header's title and the content line up on one left edge as you move between pages. Under `lg`
 * the band grows and the actions wrap under the title instead of crowding it.
 */
export function PageFrame({ children, width = 'full', fill, back, description, actions, ...header }: Props) {
    // The phone top bar carries the bell there; one control per screen.
    const mobile = useMobile()
    return (
        <div className="flex min-h-0 flex-1 flex-col">
            <div className="relative flex min-h-76 shrink-0 items-center px-16 py-12 sm:px-24 lg:h-76 lg:py-0">
                <FadeRule className="pointer-events-none absolute inset-x-0 bottom-0" />
                {back && (
                    <Link
                        to={back.href}
                        state={back.state}
                        aria-label={`Back to ${back.label}`}
                        title={`Back to ${back.label}`}
                        className="-ml-8 mr-8 grid size-32 shrink-0 place-items-center self-center rounded-md text-muted hover:bg-raised hover:text-fg"
                    >
                        <Icon name="chevron-left" size={17} />
                    </Link>
                )}
                <PageHeader
                    {...header}
                    actions={
                        <div className="flex flex-wrap items-center gap-10">
                            {actions}
                            {!mobile && <NotificationsBell />}
                        </div>
                    }
                    className="min-w-0 flex-1 lg:flex-nowrap! lg:items-center!"
                />
            </div>
            {fill ? (
                <div className="flex min-h-0 flex-1 flex-col">{children}</div>
            ) : (
                <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
                    <div className={`flex w-full flex-col gap-24 px-16 py-20 sm:px-24 sm:py-24 ${width === 'narrow' ? 'max-w-3xl' : ''}`}>
                        {description && <p className="type-ui-para text-muted">{description}</p>}
                        {children}
                    </div>
                </div>
            )}
        </div>
    )
}
