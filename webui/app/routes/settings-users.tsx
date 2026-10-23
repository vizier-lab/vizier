import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Users">
            <EmptyState kind="empty" title="Users" message="This page is being rebuilt." />
        </PageFrame>
    )
}
