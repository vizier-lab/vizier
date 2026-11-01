import { Badge, Button, EmptyState, Eyebrow, FileRow, FilterTabs, KeyValueList, Skeleton, CodeBlock } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router'
import { ApiError } from '../api/client'
import { deleteSkill, getSkill, type Skill } from '../api/endpoints/skills'
import { relativeTime } from '../lib/format'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { Markdown } from '../screens/shared/Markdown'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { toast } from '../stores/toasts'

/** One skill: its SKILL.md as it reads, with what invokes it beside it. */
export default function SkillPage() {
    const { agentId = '', slug = '' } = useParams()
    const [params] = useSearchParams()
    const navigate = useNavigate()
    const scope = params.get('scope') === 'global' ? 'global' : 'agent'
    const [skill, setSkill] = useState<Skill | null>(null)
    const [missing, setMissing] = useState(false)
    const [view, setView] = useState<'read' | 'source'>('read')
    const [deleting, setDeleting] = useState(false)

    useEffect(() => {
        getSkill(scope, agentId, slug).then(setSkill, (e) => {
            if (e instanceof ApiError && e.status === 404) setMissing(true)
            else toast.error("Couldn't load the skill", e instanceof Error ? e.message : undefined)
        })
    }, [agentId, slug, scope])

    const remove = async () => {
        try {
            await deleteSkill(scope, agentId, slug)
            toast.success('Skill deleted')
            navigate(`/${agentId}/skills`)
        } catch (e) {
            setDeleting(false)
            toast.error("Couldn't delete the skill", e instanceof Error ? e.message : undefined)
        }
    }

    const back = { href: `/${agentId}/skills`, label: 'Skills' }
    if (missing) {
        return (
            <PageFrame title={`/${slug}`} back={back}>
                <EmptyState kind="error" title="This skill isn’t there" message="It may have been deleted, or it belongs to another scope. Every skill is listed on Skills." />
            </PageFrame>
        )
    }

    return (
        <PageFrame
            fill
            back={back}
            title={`/${slug}`}
            meta={skill ? `${scope === 'agent' ? 'This agent' : 'Global'} · version ${skill.version}${skill.updated ? ` · updated ${relativeTime(skill.updated)}` : ''}` : undefined}
            actions={
                <>
                    <Button variant="ghost" icon="trash" disabled={!skill} onClick={() => setDeleting(true)}>
                        Delete
                    </Button>
                    <Button variant="primary" icon="pencil" disabled={!skill} onClick={() => navigate(`/${agentId}/skills/${encodeURIComponent(slug)}/edit?scope=${scope}`)}>
                        Edit
                    </Button>
                </>
            }
        >
            <Panes
                paneLabel="About this skill"
                pane={
                    skill ? (
                        <>
                            <p className="type-body-para text-fg">{skill.description}</p>
                            <KeyValueList
                                items={[
                                    { label: 'Invoke', value: <span className="type-mono-sm">/{skill.name} [input]</span> },
                                    { label: 'Scope', value: <Badge tone={scope === 'agent' ? 'live' : 'neutral'}>{scope === 'agent' ? 'This agent' : 'Global'}</Badge> },
                                    { label: 'Version', value: skill.version },
                                ]}
                            />
                            {skill.keywords.length > 0 && (
                                <section className="flex flex-col gap-8">
                                    <Eyebrow as="h3" count={skill.keywords.length}>
                                        Keywords
                                    </Eyebrow>
                                    <span className="flex flex-wrap gap-4">
                                        {skill.keywords.map((k) => (
                                            <Badge key={k}>{k}</Badge>
                                        ))}
                                    </span>
                                </section>
                            )}
                            {skill.resources.length > 0 && (
                                <section className="flex flex-col gap-8">
                                    <Eyebrow as="h3" count={skill.resources.length}>
                                        Resources
                                    </Eyebrow>
                                    <div className="-mx-8 flex flex-col">
                                        {skill.resources.map((r) => (
                                            <FileRow key={r} name={r} />
                                        ))}
                                    </div>
                                </section>
                            )}
                        </>
                    ) : (
                        <Skeleton lines={5} />
                    )
                }
            >
                <div className="flex items-center gap-12">
                    <span className="flex-1 type-mono-sm text-muted">SKILL.md</span>
                    <FilterTabs
                        aria-label="Show"
                        value={view}
                        onChange={setView}
                        options={[
                            { value: 'read', label: 'Preview' },
                            { value: 'source', label: 'Source' },
                        ]}
                    />
                </div>
                {!skill ? (
                    <Skeleton lines={14} />
                ) : view === 'read' ? (
                    <article className="max-w-[72ch]">
                        <Markdown size="document">{skill.content}</Markdown>
                    </article>
                ) : (
                    <CodeBlock code={skill.content} language="markdown" filename="SKILL.md" className="max-w-[100ch]" />
                )}
            </Panes>

            <ConfirmDialog
                open={deleting}
                title={`Delete /${slug}?`}
                removes={['The skill and its resources']}
                confirmLabel="Delete skill"
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(false)}
            />
        </PageFrame>
    )
}
