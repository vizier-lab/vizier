import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Roles">
            <EmptyState kind="empty" title="Roles" message="This page is being rebuilt." />
        </PageFrame>
    )
}
