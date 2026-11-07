import { useEffect } from 'react'
import { Navigate, useParams } from 'react-router'
import { useWorkspaces } from '../stores/workspaces'
import { WorkspaceTopic } from '../screens/workspaces/WorkspaceTopic'

/** `/workspaces/:workspaceId` opens its first topic; `/workspaces/:workspaceId/:topicId` is the conversation. */
export default function WorkspaceRoute() {
    const { workspaceId = '', topicId } = useParams()
    const { list, loaded, load } = useWorkspaces()
    useEffect(() => {
        if (!loaded) void load()
    }, [loaded, load])
    const workspace = list.find((w) => w.id === workspaceId)
    if (!topicId) {
        const first = workspace?.topics[0]
        return first ? <Navigate to={`/workspaces/${workspaceId}/${first.id}`} replace /> : null
    }
    return <WorkspaceTopic key={`${workspaceId}/${topicId}`} workspaceId={workspaceId} topicId={topicId} />
}
