import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Home">
            <EmptyState kind="empty" title="Home" message="This page is being rebuilt." />
        </PageFrame>
    )
}
