import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="API keys">
            <EmptyState kind="empty" title="API keys" message="This page is being rebuilt." />
        </PageFrame>
    )
}
