import { Badge, Button } from '@vizier/design'
import { useNavigate, useParams } from 'react-router'
import type { Skill } from '../api/endpoints/skills'
import { relativeTime } from '../lib/format'
import { useMobile } from '../lib/useMobile'
import { usePagedList } from '../lib/usePagedList'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'

const SORTS = [
    { value: 'name', label: 'Name' },
    { value: 'updated', label: 'Recently updated' },
]

const skillHref = (agentId: string, s: Pick<Skill, 'name' | 'scope'>) => `/${agentId}/skills/${encodeURIComponent(s.name)}?scope=${s.scope}`

/** Every skill the agent can use, its own and the global ones, across the page. A skill opens on its own page. */
export default function Skills() {
    const { agentId = '' } = useParams()
    const navigate = useNavigate()
    const list = usePagedList<Skill>(`/agents/${agentId}/skills?scope=all`, { sort: 'name' })
    const counts = list.data?.counts
    const create = () => navigate(`/${agentId}/skills/new/edit?scope=agent`)
    // A phone keeps the name and scope; the rest is on the skill's page.
    const mobile = useMobile()
    const keep = <T,>(cells: T[]) => (mobile ? cells.slice(0, 2) : cells)

    return (
        <PageFrame
            title="Skills"
            meta={counts ? `${counts.all ?? 0} skill${counts.all === 1 ? '' : 's'} · ${counts.agent ?? 0} on this agent · ${counts.global ?? 0} global` : undefined}
            actions={
                <Button variant="primary" icon="plus" onClick={create}>
                    New skill
                </Button>
            }
        >
            <PagedTable<Skill>
                label="Skills"
                searchLabel="Search skills"
                list={list}
                rowKey={(s) => `${s.scope}:${s.name}`}
                onSelect={(s) => navigate(skillHref(agentId, s))}
                sorts={SORTS}
                filters={[
                    { value: 'all', label: 'All' },
                    { value: 'agent', label: 'This agent' },
                    { value: 'global', label: 'Global' },
                ]}
                columns={keep([{ label: 'Skill' }, { label: 'Scope', width: 120 }, { label: 'Keywords', width: 320 }, { label: 'Updated', width: 130 }])}
                row={(s) =>
                    keep([
                        <span key="n" className="flex min-w-0 flex-col">
                            <span className="truncate type-mono-sm text-fg">/{s.name}</span>
                            <span className="truncate type-small text-muted">{s.description}</span>
                        </span>,
                        <Badge key="s" tone={s.scope === 'agent' ? 'live' : 'neutral'}>{s.scope === 'agent' ? 'This agent' : 'Global'}</Badge>,
                        <span key="k" className="flex flex-wrap gap-4">
                            {s.keywords.slice(0, 4).map((k) => (
                                <Badge key={k}>{k}</Badge>
                            ))}
                        </span>,
                        <span key="u" className="type-mono-xs text-faint">{s.updated ? relativeTime(s.updated) : '–'}</span>,
                    ])
                }
                empty={{
                    icon: 'skills',
                    title: 'No skills yet',
                    message: 'A skill is a reusable behaviour the agent, or you, can invoke by name.',
                    action: (
                        <Button variant="primary" icon="plus" onClick={create}>
                            New skill
                        </Button>
                    ),
                }}
            />
        </PageFrame>
    )
}
