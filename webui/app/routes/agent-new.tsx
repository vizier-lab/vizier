import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="New agent">
            <EmptyState kind="empty" title="New agent" message="This page is being rebuilt." />
        </PageFrame>
    )
}
