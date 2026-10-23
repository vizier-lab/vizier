import { Navigate, useParams } from 'react-router'

/** Integrations live on the agent's Config now. */
export default function IntegrationsAddonsRedirect() {
    const { agentId = '' } = useParams()
    return <Navigate replace to={`/${agentId}/config?tab=integrations&section=addons`} />
}
