import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Addons">
            <EmptyState kind="empty" title="Addons" message="This page is being rebuilt." />
        </PageFrame>
    )
}
