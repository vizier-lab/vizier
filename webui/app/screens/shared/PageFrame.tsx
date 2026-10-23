import { PageHeader, type PageHeaderProps } from '@vizier/design'
import type { ReactNode } from 'react'

type Props = Omit<PageHeaderProps, 'children'> & {
    children?: ReactNode
    /** Content width: the design's reading column, or the full page. */
    width?: 'narrow' | 'wide' | 'full'
}

const WIDTH = { narrow: 'max-w-3xl', wide: 'max-w-6xl', full: '' } as const

/** The standard page body: a PageHeader over a content area at the design's width. */
export function PageFrame({ children, width = 'wide', ...header }: Props) {
    return (
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
            <div className={`mx-auto flex w-full flex-col gap-24 px-32 py-32 ${WIDTH[width]}`}>
                <PageHeader {...header} />
                {children}
            </div>
        </div>
    )
}
