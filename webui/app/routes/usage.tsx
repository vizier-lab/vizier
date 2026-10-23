import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Usage">
            <EmptyState kind="empty" title="Usage" message="This page is being rebuilt." />
        </PageFrame>
    )
}
