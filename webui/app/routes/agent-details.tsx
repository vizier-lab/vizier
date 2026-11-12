import { AgentDetailsList, Sigil, Status, agentStatusLabel } from '@vizier/design'
import { Navigate, useParams } from 'react-router'
import { agentState } from '../api/endpoints/agents'
import { canManageSecrets, canViewArtifacts } from '../lib/access'
import { detailsAccess, detailsHref } from '../lib/agentDetails'
import { assetSrc } from '../lib/fileUrls'
import { relativeTime } from '../lib/format'
import { useMobile } from '../lib/useMobile'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'

/**
 * Everything about one agent, on a phone (spec 056): reached from the button beside the agent in
 * the top bar or in the switcher. A wide screen has the agent column and its menu for this, so
 * there the address goes to the agent's config, its nearest page.
 */
export default function AgentDetails() {
    const { agentId = '' } = useParams()
    const mobile = useMobile()
    const { canEditAgent } = useSession()
    const { agents, health } = useAgents()
    const agent = agents.find((a) => a.agent_id === agentId)

    if (!mobile) return <Navigate to={`/${agentId}/config`} replace />

    const h = health[agentId]
    const state = agentState(h)
    const { hidden, disabled } = detailsAccess({
        viewArtifacts: agent ? canViewArtifacts(agent) : false,
        manageSecrets: agent ? canManageSecrets(agent) : false,
        editAgent: agent ? canEditAgent(agent) : false,
    })

    return (
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
            <div className="flex flex-col gap-24 px-16 pt-12 pb-24">
                <section aria-label="Agent" className="flex items-center gap-14">
                    <Sigil
                        agentId={agentId}
                        size="lg"
                        image={assetSrc(agent?.avatar_url)}
                        live={state === 'live' || state === 'running'}
                        label={agent?.name ?? agentId}
                    />
                    <div className="flex min-w-0 flex-col gap-4">
                        <h1 className="truncate type-title text-fg">{agent?.name ?? agentId}</h1>
                        <Status
                            state={state}
                            font="mono-xs"
                            dot={false}
                            label={agentStatusLabel(state)}
                            detail={h?.next_retry_at ? `retry ${relativeTime(h.next_retry_at)}` : undefined}
                        />
                        <span className="truncate type-mono-xs text-faint">{agentId}</span>
                    </div>
                </section>
                <AgentDetailsList
                    hrefFor={(page) => detailsHref(agentId, page)}
                    hiddenPages={hidden}
                    disabledPages={disabled}
                />
            </div>
        </div>
    )
}
