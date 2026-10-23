import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Skills">
            <EmptyState kind="empty" title="Skills" message="This page is being rebuilt." />
        </PageFrame>
    )
}
