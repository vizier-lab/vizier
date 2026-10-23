import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Chat">
            <EmptyState kind="empty" title="Chat" message="This page is being rebuilt." />
        </PageFrame>
    )
}
