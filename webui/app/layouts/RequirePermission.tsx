import { EmptyState } from '@vizier/design'
import type { ReactNode } from 'react'
import { useCan } from '../lib/access'
import { PageFrame } from '../screens/shared/PageFrame'

/** FR-012: a page the person can't use says so, rather than loading and failing. */
export function RequirePermission({ permission, children }: { permission: string; children: ReactNode }) {
    const allowed = useCan(permission)
    if (allowed) return <>{children}</>
    return (
        <PageFrame title="No access">
            <EmptyState kind="empty" icon="lock" title="You don't have access to this page" message="Ask an administrator for the permission it needs." />
        </PageFrame>
    )
}
