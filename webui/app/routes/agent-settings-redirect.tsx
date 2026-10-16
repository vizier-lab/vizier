import { Navigate, useParams } from 'react-router'

/** `/:agentId/settings` moved to Misc › Config (`/:agentId/config`). */
export default function AgentSettingsRedirect() {
  const { agentId } = useParams()
  return <Navigate to={`/${agentId}/config`} replace />
}
