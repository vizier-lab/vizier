import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="MCP">
            <EmptyState kind="empty" title="MCP" message="This page is being rebuilt." />
        </PageFrame>
    )
}
