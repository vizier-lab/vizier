import { Badge, Button, CodeBlock, FileRow, KeyValueList, PanelHeader, TextLink } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { deleteSkill, type Skill } from '../api/endpoints/skills'
import { relativeTime } from '../lib/format'
import { usePagedList } from '../lib/usePagedList'
import { SplitView } from '../screens/shared/SplitView'
import { useWide } from '../lib/useWide'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'name', label: 'Name' },
    { value: 'updated', label: 'Recently updated' },
]

export default function Skills() {
    const { agentId = '' } = useParams()
    const navigate = useNavigate()
    const [params, setParams] = useSearchParams()
    const list = usePagedList<Skill>(`/agents/${agentId}/skills?scope=all`, { sort: 'name' })
    const selectedName = params.get('skill')
    const selected = list.data?.items.find((s) => s.name === selectedName) ?? null
    const [deleting, setDeleting] = useState<Skill | null>(null)

    const select = (name: string | null) =>
        setParams(
            (p) => {
                const next = new URLSearchParams(p)
                if (name) next.set('skill', name)
                else next.delete('skill')
                return next
            },
            { replace: true }
        )

    // Beside the list the detail is always there: the first skill until another is chosen.
    const wide = useWide()
    useEffect(() => {
        const first = list.data?.items[0]
        if (wide && !selectedName && first) select(first.name)
    }, [wide, selectedName, list.data]) // eslint-disable-line react-hooks/exhaustive-deps

    const remove = async () => {
        if (!deleting) return
        try {
            await deleteSkill(deleting.scope, agentId, deleting.name)
            toast.success('Skill deleted')
            if (selectedName === deleting.name) select(null)
            setDeleting(null)
            list.reload()
        } catch (e) {
            toast.error("Couldn't delete the skill", e instanceof Error ? e.message : undefined)
        }
    }

    const counts = list.data?.counts
    const edit = (s: Skill) => navigate(`/${agentId}/skills/${encodeURIComponent(s.name)}/edit?scope=${s.scope}`)

    // With the detail beside it the list keeps just the name and scope.
    const keep = <T,>(cells: T[]) => (wide && selected ? cells.slice(0, 2) : cells)

    const detail = selected && (
        <div className="flex flex-col gap-16">
                            <div className="flex items-center gap-8">
                                <Badge tone={selected.scope === 'agent' ? 'live' : 'neutral'}>{selected.scope === 'agent' ? 'This agent' : 'Global'}</Badge>
                                <span className="flex-1" />
                                <Button size="sm" variant="secondary" onClick={() => edit(selected)}>
                                    Edit
                                </Button>
                                <Button size="sm" variant="destructive" onClick={() => setDeleting(selected)}>
                                    Delete
                                </Button>
                            </div>
                            <p className="type-body text-fg">{selected.description}</p>
                            <KeyValueList
                                items={[
                                    { label: 'Version', value: selected.version },
                                    { label: 'Keywords', value: <span className="flex flex-wrap justify-end gap-4">{selected.keywords.map((k) => <Badge key={k}>{k}</Badge>)}</span> },
                                    { label: 'Invoke', value: <span className="type-mono-sm">/{selected.name} [input]</span> },
                                ]}
                            />
                            <section className="flex flex-col gap-8">
                                <PanelHeader title="SKILL.md" level={3} />
                                <CodeBlock code={selected.content} language="markdown" filename="SKILL.md" />
                            </section>
                            {selected.resources.length > 0 && (
                                <section className="flex flex-col gap-8">
                                    <PanelHeader title="Resources" meta={`${selected.resources.length}`} level={3} />
                                    {selected.resources.map((r) => (
                                        <FileRow key={r} name={r} />
                                    ))}
                                </section>
                            )}
                            <TextLink onClick={() => edit(selected)}>Edit in the editor</TextLink>
                        </div>
    )

    return (
        <PageFrame
            title="Skills"
            meta={counts ? `${counts.all ?? 0} skill${counts.all === 1 ? '' : 's'} · ${counts.agent ?? 0} on this agent · ${counts.global ?? 0} global` : undefined}
            actions={
                <Button variant="primary" icon="plus" onClick={() => navigate(`/${agentId}/skills/new/edit?scope=agent`)}>
                    New skill
                </Button>
            }
        >
            <SplitView title={selected ? `/${selected.name}` : ''} detail={selected ? detail : undefined} onClose={() => select(null)}>
            <PagedTable<Skill>
                label="Skills"
                searchLabel="Search skills"
                list={list}
                rowKey={(s) => `${s.scope}:${s.name}`}
                selectedKey={selected ? `${selected.scope}:${selected.name}` : undefined}
                onSelect={(s) => select(s.name)}
                sorts={SORTS}
                filters={[
                    { value: 'all', label: 'All' },
                    { value: 'agent', label: 'This agent' },
                    { value: 'global', label: 'Global' },
                ]}
                columns={keep([{ label: 'Skill' }, { label: 'Scope', width: 120 }, { label: 'Keywords', width: 260 }, { label: 'Updated', width: 130 }])}
                row={(s) => keep([
                    <span key="n" className="flex min-w-0 flex-col">
                        <span className="truncate type-mono-sm text-fg">/{s.name}</span>
                        <span className="truncate type-small text-muted">{s.description}</span>
                    </span>,
                    <Badge key="s" tone={s.scope === 'agent' ? 'live' : 'neutral'}>{s.scope === 'agent' ? 'This agent' : 'Global'}</Badge>,
                    <span key="k" className="flex flex-wrap gap-4">
                        {s.keywords.slice(0, 3).map((k) => (
                            <Badge key={k}>{k}</Badge>
                        ))}
                    </span>,
                    <span key="u" className="type-mono-xs text-faint">{s.updated ? relativeTime(s.updated) : '–'}</span>,
                ])}
                empty={{
                    icon: 'skills',
                    title: 'No skills yet',
                    message: 'A skill is a reusable behaviour the agent, or you, can invoke by name.',
                    action: (
                        <Button variant="primary" icon="plus" onClick={() => navigate(`/${agentId}/skills/new/edit?scope=agent`)}>
                            New skill
                        </Button>
                    ),
                }}
            />
            </SplitView>

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete /${deleting?.name ?? ''}?`}
                removes={['The skill and its resources']}
                confirmLabel="Delete skill"
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(null)}
            />
        </PageFrame>
    )
}
