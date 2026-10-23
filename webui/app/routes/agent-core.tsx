import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Core">
            <EmptyState kind="empty" title="Core" message="This page is being rebuilt." />
        </PageFrame>
    )
}
