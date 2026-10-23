import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Set up Vizier">
            <EmptyState kind="empty" title="Set up Vizier" message="This page is being rebuilt." />
        </PageFrame>
    )
}
