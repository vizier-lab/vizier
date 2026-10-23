import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Dreams">
            <EmptyState kind="empty" title="Dreams" message="This page is being rebuilt." />
        </PageFrame>
    )
}
