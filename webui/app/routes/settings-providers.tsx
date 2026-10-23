import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Providers">
            <EmptyState kind="empty" title="Providers" message="This page is being rebuilt." />
        </PageFrame>
    )
}
