import { EmptyState } from '@vizier/design'
import { PageFrame } from '../screens/shared/PageFrame'

export default function Route() {
    return (
        <PageFrame title="Skill editor">
            <EmptyState kind="empty" title="Skill editor" message="This page is being rebuilt." />
        </PageFrame>
    )
}
