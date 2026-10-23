import { PageHeader, type PageHeaderProps } from '@vizier/design'
import type { ReactNode } from 'react'

type Props = Omit<PageHeaderProps, 'children' | 'size'> & { children: ReactNode; width?: 'narrow' | 'wide' }

/** A Settings page: the settings-size header with its sentence of description over the content. */
export function SettingsFrame({ children, width = 'narrow', ...header }: Props) {
    return (
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
            <div className={`mx-auto flex w-full flex-col gap-24 px-16 py-20 sm:px-32 sm:py-32 ${width === 'wide' ? 'max-w-6xl' : 'max-w-4xl'}`}>
                <PageHeader size="settings" {...header} />
                {children}
            </div>
        </div>
    )
}
