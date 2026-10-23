import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Password">
            <EmptyState kind="empty" title="Password" message="This page is being rebuilt." />
        </PageFrame>
    )
}
