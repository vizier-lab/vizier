import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Profile">
            <EmptyState kind="empty" title="Profile" message="This page is being rebuilt." />
        </PageFrame>
    )
}
