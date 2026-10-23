import type { PageHeaderProps } from '@vizier/design'
import type { ReactNode } from 'react'
import { PageFrame } from './PageFrame'

type Props = Omit<PageHeaderProps, 'children' | 'size'> & { children: ReactNode; width?: 'narrow' | 'wide' | 'full' }

/** A Settings page: the standard page, with its sentence of description at the top of the content. */
export function SettingsFrame({ children, width = 'narrow', ...header }: Props) {
    return (
        <PageFrame width={width} {...header}>
            {children}
        </PageFrame>
    )
}
