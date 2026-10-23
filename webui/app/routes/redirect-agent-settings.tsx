import { Navigate, useParams } from 'react-router'

export default function RedirectAgentSettings() {
    const { agentId } = useParams()
    return <Navigate to={`/${agentId}/config`} replace />
}
