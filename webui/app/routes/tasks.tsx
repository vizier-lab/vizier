import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Tasks">
            <EmptyState kind="empty" title="Tasks" message="This page is being rebuilt." />
        </PageFrame>
    )
}
