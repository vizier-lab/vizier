import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Channels">
            <EmptyState kind="empty" title="Channels" message="This page is being rebuilt." />
        </PageFrame>
    )
}
