import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Config">
            <EmptyState kind="empty" title="Config" message="This page is being rebuilt." />
        </PageFrame>
    )
}
