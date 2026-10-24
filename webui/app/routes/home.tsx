import { AgentCard, Button, Callout, EmptyState, FilterTabs, LoadMore, PageHeader, SearchField, SetupStepCard } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { paged } from '../api/client'
import { agentState, type Agent, type AgentHealth } from '../api/endpoints/agents'
import { count, relativeTime } from '../lib/format'
import { Enter, stagger } from '../lib/motion'
import { usePagedList } from '../lib/usePagedList'
import { getOverview, type Overview } from '../api/endpoints/overview'
import { greetingMeta } from '../lib/overview'
import { useMobile } from '../lib/useMobile'
import { MobileHome } from '../screens/home/MobileHome'
import { ActivityPanel } from '../screens/home/ActivityPanel'
import { AwayFeed } from '../screens/home/AwayFeed'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'

type AgentRow = Agent & { last_active?: string }

const COURT = 6

const cardState = agentState

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
                    { value: 'live', label: 'Running', count: list.data ? count(list.data.counts.live ?? 0) : undefined },
                    { value: 'failed', label: 'Failed', count: list.data ? count(list.data.counts.failed ?? 0) : undefined },
                ]}
            />
            {list.data && items.length === 0 ? (
                <EmptyState kind="no-results" title="No agents match" message="Try a different search or filter." />
            ) : (
                <div className="grid grid-cols-1 gap-12 md:grid-cols-2 xl:grid-cols-3">
                    {items.map((a, i) => (
                        <Enter key={a.agent_id} delay={stagger(i, 0.025)} className="h-full">
                            <Card agent={a} health={health[a.agent_id]} />
                        </Enter>
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
    const navigate = useNavigate()
    const agents = useAgents((s) => s.agents)
    const loaded = useAgents((s) => s.loaded)
    const health = useAgents((s) => s.health)
    const user = useSession((s) => s.user)
    const canCreate = useSession((s) => s.can('all_agents:create'))
    const [recent, setRecent] = useState<AgentRow[] | null>(null)
    const [showAll, setShowAll] = useState(false)
    const [hasProvider, setHasProvider] = useState(false)
    const [overview, setOverview] = useState<Overview>()

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
        let alive = true
        const load = () => getOverview().then((o) => alive && setOverview(o), () => {})
        void load()
        const id = setInterval(() => document.visibilityState === 'visible' && void load(), 30_000)
        return () => {
            alive = false
            clearInterval(id)
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

    const hour = new Date().getHours()
    const hello = user ? `Good ${hour < 12 ? 'morning' : hour < 18 ? 'afternoon' : 'evening'}, ${user.username}.` : 'Welcome'
    const firstRun = loaded && agents.length === 0
    const mobile = useMobile()

    if (mobile && !firstRun && !showAll) {
        return <MobileHome greeting={hello} overview={overview} agents={recent ?? []} health={health} total={agents.length} onShowAll={() => setShowAll(true)} />
    }

    return (
        <div className="flex min-h-0 flex-1">
            <div className="flex min-w-0 flex-1 flex-col">
                <div className="flex h-76 shrink-0 items-center border-b border-border px-24">
                    <PageHeader
                        className="w-full flex-nowrap! items-center!"
                        title={hello}
                        meta={overview ? greetingMeta(overview) : undefined}
                        actions={
                            canCreate ? (
                                <Button variant="primary" icon="plus" onClick={() => navigate('/agents/new')}>
                                    New agent
                                </Button>
                            ) : undefined
                        }
                    />
                </div>
                <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
                <div className="mx-auto flex w-full max-w-5xl flex-col gap-24 px-24 py-24">
                    <ActivityPanel />
                    {firstRun ? (
                        <FirstRun hasProvider={hasProvider} />
                    ) : showAll ? (
                        <AllAgents health={health} />
                    ) : (
                        <section id="the-court" aria-label="The court" className="flex flex-col gap-16">
                            <h2 className="type-ui-strong text-fg">The court</h2>
                            <div className="grid grid-cols-1 gap-12 md:grid-cols-2 xl:grid-cols-3">
                                {(recent ?? []).map((a) => (
                                    <Card key={a.agent_id} agent={a} health={health[a.agent_id]} />
                                ))}
                            </div>
                            {agents.length > COURT && (
                                <button
                                    type="button"
                                    onClick={() => setShowAll(true)}
                                    className="rounded-lg border border-dashed border-border-strong px-16 py-14 type-ui text-muted hover:text-fg"
                                >
                                    Show all {count(agents.length)} agents <span className="text-faint">sorted by last active</span>
                                </button>
                            )}
                        </section>
                    )}
                </div>
                </div>
            </div>
            <div className="hidden w-340 shrink-0 overflow-y-auto border-l border-border px-24 py-32 xl:block">
                <AwayFeed />
            </div>
        </div>
    )
}
