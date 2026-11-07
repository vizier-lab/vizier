import { useNavigate, useParams, useLocation } from 'react-router'
import { useWorkspaces } from '../../stores/workspaces'
import { WorkspaceSidebar } from './WorkspaceSidebar'

/** The sidebar in a Workspace: its topics and members. Empty until the list has loaded. */
export function WorkspaceColumn() {
    const { workspaceId, topicId } = useParams()
    const navigate = useNavigate()
    const { pathname } = useLocation()
    const workspace = useWorkspaces((s) => s.list.find((w) => w.id === workspaceId))
    if (!workspace) return null
    return <WorkspaceSidebar workspace={workspace} activeTopic={topicId} onMembers={() => navigate(`${pathname}?members=1`)} />
}
