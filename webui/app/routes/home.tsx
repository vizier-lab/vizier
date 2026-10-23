import { AgentCard, Button, Callout, EmptyState, FilterTabs, LoadMore, SearchField, SetupStepCard } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import { paged } from '../api/client'
import type { Agent, AgentHealth } from '../api/endpoints/agents'
import { count, relativeTime } from '../lib/format'
import { usePagedList } from '../lib/usePagedList'
import { PageFrame } from '../screens/shared/PageFrame'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'

type AgentRow = Agent & { last_active?: string }

const COURT = 6

function cardState(h?: AgentHealth) {
    if (!h) return 'idle' as const
    if (h.state === 'failed') return h.next_retry_at ? ('retrying' as const) : ('failed' as const)
    if (h.state === 'starting') return 'starting' as const
    if (h.state === 'stopped') return 'stopped' as const
    return 'idle' as const
}

function Card({ agent, health }: { agent: AgentRow; health?: AgentHealth }) {
    const state = cardState(health)
    const detail =
        state === 'retrying' && health?.next_retry_at
            ? `retry ${relativeTime(health.next_retry_at)}`
            : state === 'failed'
              ? 'failed'
              : agent.last_active
                ? relativeTime(agent.last_active)
                : 'never active'
    return (
        <AgentCard
            agentId={agent.agent_id}
            name={agent.name}
            description={agent.description}
            state={state}
            detail={detail}
            model={health?.model ? `${health.model.provider}/${health.model.model}` : undefined}
            owned={false}
            href={`/${agent.agent_id}/chat`}
        />
    )
}

/** Every agent, paged from the server: search, Mine / Live / Failed. */
function AllAgents({ health }: { health: Record<string, AgentHealth> }) {
    const list = usePagedList<AgentRow>('/agents', { sort: 'last_active' }, 24)
    const items = list.data?.items ?? []
    return (
        <section aria-label="All agents" className="flex flex-col gap-16">
            <SearchField
                aria-label="Search agents"
                placeholder="Search agents"
                value={list.search}
                onValueChange={list.setSearch}
                count={list.data ? count(list.data.total) : undefined}
            />
            <FilterTabs
                aria-label="Filter agents"
                value={list.state.filter}
                onChange={list.setFilter}
                options={[
                    { value: 'all', label: 'All', count: list.data ? count(list.data.counts.all ?? 0) : undefined },
                    { value: 'mine', label: 'Mine', count: list.data ? count(list.data.counts.mine ?? 0) : undefined },
                    { value: 'live', label: 'Live', count: list.data ? count(list.data.counts.live ?? 0) : undefined },
                    { value: 'failed', label: 'Failed', count: list.data ? count(list.data.counts.failed ?? 0) : undefined },
                ]}
            />
            {list.data && items.length === 0 ? (
                <EmptyState kind="no-results" title="No agents match" message="Try a different search or filter." />
            ) : (
                <div className="grid grid-cols-1 gap-12 md:grid-cols-2 xl:grid-cols-3">
                    {items.map((a) => (
                        <Card key={a.agent_id} agent={a} health={health[a.agent_id]} />
                    ))}
                </div>
            )}
            {list.data && list.data.total > items.length && (
                <LoadMore label="Next page" remaining={list.data.total - items.length} onLoad={() => list.setPage(list.state.page + 1)} />
            )}
        </section>
    )
}

function FirstRun({ hasProvider }: { hasProvider: boolean }) {
    const navigate = useNavigate()
    const can = useSession((s) => s.can)
    return (
        <div className="flex flex-col gap-24">
            <ol className="flex flex-col gap-12">
                <SetupStepCard
                    step={1}
                    state={hasProvider ? 'done' : 'current'}
                    title="Connect a model provider"
                    description="Agents need a model to think with. Add a provider and its key."
                    action={
                        <Button variant={hasProvider ? 'secondary' : 'primary'} onClick={() => navigate('/settings/providers')}>
                            {hasProvider ? 'Manage' : 'Add a provider'}
                        </Button>
                    }
                />
                <SetupStepCard
                    step={2}
                    state={hasProvider ? 'current' : 'upcoming'}
                    title="Create your first agent"
                    description="Give it a name, a model and the tools it may use."
                    action={
                        can('all_agents:create') && (
                            <Button variant={hasProvider ? 'primary' : 'secondary'} onClick={() => navigate('/agents/new')}>
                                Create agent
                            </Button>
                        )
                    }
                />
                <SetupStepCard step={3} state="upcoming" title="Say hello" description="Open its chat and send a first message." />
            </ol>
            <Callout title="No API key?">Ollama runs open models on this machine, with no key and no account.</Callout>
        </div>
    )
}

export default function Home() {
    const agents = useAgents((s) => s.agents)
    const loaded = useAgents((s) => s.loaded)
    const health = useAgents((s) => s.health)
    const user = useSession((s) => s.user)
    const [recent, setRecent] = useState<AgentRow[] | null>(null)
    const [showAll, setShowAll] = useState(false)
    const [hasProvider, setHasProvider] = useState(false)

    useEffect(() => {
        let alive = true
        paged<AgentRow>('/agents', { limit: COURT, sort: 'last_active' })
            .then((p) => alive && setRecent(p.items))
            .catch(() => alive && setRecent([]))
        return () => {
            alive = false
        }
    }, [agents.length])

    useEffect(() => {
        if (agents.length > 0) return
        import('../api/client').then(({ api }) =>
            api
                .get<unknown[]>('/providers')
                .then((p) => setHasProvider(Array.isArray(p) && p.some((x) => (x as { configured?: boolean }).configured !== false)))
                .catch(() => {})
        )
    }, [agents.length])

    const failed = useMemo(() => Object.values(health).filter((h) => h.state === 'failed').length, [health])
    const hello = user ? `Good ${new Date().getHours() < 12 ? 'morning' : new Date().getHours() < 18 ? 'afternoon' : 'evening'}, ${user.username}` : 'Welcome'

    return (
        <PageFrame
            title={hello}
            meta={loaded ? `${count(agents.length)} agent${agents.length === 1 ? '' : 's'} at court${failed ? ` · ${failed} failed` : ''}` : undefined}
        >
            {loaded && agents.length === 0 ? (
                <FirstRun hasProvider={hasProvider} />
            ) : showAll ? (
                <AllAgents health={health} />
            ) : (
                <section aria-label="The court" className="flex flex-col gap-16">
                    <div className="grid grid-cols-1 gap-12 md:grid-cols-2 xl:grid-cols-3">
                        {(recent ?? []).map((a) => (
                            <Card key={a.agent_id} agent={a} health={health[a.agent_id]} />
                        ))}
                    </div>
                    {agents.length > COURT && (
                        <div>
                            <Button variant="secondary" onClick={() => setShowAll(true)}>
                                Show all {count(agents.length)} agents
                            </Button>
                        </div>
                    )}
                </section>
            )}
        </PageFrame>
    )
}
