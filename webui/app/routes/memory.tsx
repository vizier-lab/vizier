import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Memory">
            <EmptyState kind="empty" title="Memory" message="This page is being rebuilt." />
        </PageFrame>
    )
}
