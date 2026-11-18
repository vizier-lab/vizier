import { Badge, Button, CodeBlock, DiffView, EmptyState, Eyebrow, FilterTabs, KeyValueList, Notice, RevisionRow, Skeleton, Switch } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    deleteFunction,
    diffFunction,
    getFunction,
    listFunctionVersions,
    rollbackFunction,
    setFunctionEnabled,
    type FunctionDetail,
    type FunctionDiff,
    type FunctionVersion,
} from '../api/endpoints/functions'
import { canManageFunctions } from '../lib/access'
import { relativeTime } from '../lib/format'
import { authorLabel, isOutOfDate, missingCalls, nameTaken, problemText, problemsOf, schemaText } from '../lib/functions'
import { highlightCode } from '../lib/highlight'
import { diffLines } from '../lib/revisions'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { useAgents } from '../stores/agents'
import { toast } from '../stores/toasts'

const messageOf = (e: unknown, fallback: string) => (e instanceof Error && e.message ? e.message : fallback)

type View = 'script' | 'input' | 'changes'

/** One function (spec 061): its script and input, with what it calls and its history beside it.
 * People who can manage the agent's functions switch, edit, roll back and delete it. */
export default function FunctionPage() {
    const { agentId = '', name = '' } = useParams()
    const navigate = useNavigate()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const canEdit = agent ? canManageFunctions(agent) : false
    const agentName = agent?.name
    const [fn, setFn] = useState<FunctionDetail | null>(null)
    const [missing, setMissing] = useState(false)
    const [versions, setVersions] = useState<FunctionVersion[] | null>(null)
    const [view, setView] = useState<View>('script')
    const [compare, setCompare] = useState<number | null>(null)
    const [diff, setDiff] = useState<FunctionDiff | null>(null)
    const [deleting, setDeleting] = useState(false)
    const [deleteBusy, setDeleteBusy] = useState(false)

    const load = useCallback(() => {
        getFunction(agentId, name).then(setFn, (e) => {
            if (e instanceof ApiError && e.status === 404) setMissing(true)
            else toast.error('Couldn’t load the function', messageOf(e, ''))
        })
        listFunctionVersions(agentId, name).then(
            (v) => setVersions(v.versions),
            () => setVersions([])
        )
    }, [agentId, name])

    useEffect(load, [load])

    // The diff of the chosen version against the current one.
    useEffect(() => {
        if (!fn || compare === null || compare === fn.current_version) {
            setDiff(null)
            return
        }
        diffFunction(agentId, fn.id, compare, fn.current_version).then(setDiff, () => setDiff(null))
    }, [agentId, fn, compare])

    const restore = async (version: number) => {
        if (!fn) return
        try {
            const saved = await rollbackFunction(agentId, fn.id, version, fn.current_version)
            toast.success(`Version ${version} restored as version ${saved.version}`)
            setCompare(null)
            load()
        } catch (e) {
            if (isOutOfDate(e)) {
                toast.error('The function changed', 'It was reloaded; restore again if you still want to.')
                load()
            } else {
                toast.error('Couldn’t restore that version', problemsOf(e).map(problemText).join(' '))
            }
        }
    }

    const toggle = async (enabled: boolean) => {
        if (!fn) return
        try {
            await setFunctionEnabled(agentId, fn.id, enabled)
            load()
        } catch (e) {
            toast.error(`Couldn’t switch it ${enabled ? 'on' : 'off'}`, messageOf(e, ''))
        }
    }

    const remove = async () => {
        if (!fn) return
        setDeleteBusy(true)
        try {
            await deleteFunction(agentId, fn.id)
            toast.success('Function deleted')
            navigate(`/${agentId}/functions`)
        } catch (e) {
            toast.error('Couldn’t delete the function', messageOf(e, ''))
        } finally {
            setDeleteBusy(false)
            setDeleting(false)
        }
    }

    const scriptTokens = useMemo(() => (fn ? highlightCode(fn.script, 'python') : undefined), [fn])
    const schemaTokens = useMemo(() => (fn ? highlightCode(schemaText(fn.input_schema), 'json') : undefined), [fn])

    const back = { href: `/${agentId}/functions`, label: 'Functions' }
    if (missing) {
        return (
            <PageFrame title={name} back={back}>
                <EmptyState kind="error" title="This function isn’t there" message="It may have been deleted or renamed. Every function is listed on Functions." />
            </PageFrame>
        )
    }

    const attention = missingCalls(fn?.needs_attention)
    const pick = (version: number) => {
        setCompare(version)
        setView('changes')
    }
    return (
        <PageFrame
            fill
            back={back}
            title={fn?.call_as ?? name}
            meta={fn ? `${fn.category} · version ${fn.current_version} · updated ${relativeTime(fn.updated_at)}` : undefined}
            actions={
                fn &&
                canEdit && (
                    <>
                        <label className="flex items-center gap-8 type-ui text-muted">
                            <Switch checked={fn.enabled} aria-label="On" onCheckedChange={(on) => void toggle(on)} />
                            {fn.enabled ? 'On' : 'Off'}
                        </label>
                        <Button variant="ghost" icon="trash" onClick={() => setDeleting(true)}>
                            Delete
                        </Button>
                        <Button variant="primary" icon="pencil" onClick={() => navigate(`/${agentId}/functions/${encodeURIComponent(fn.name)}/edit`)}>
                            Edit
                        </Button>
                    </>
                )
            }
        >
            <Panes
                paneLabel="About this function"
                pane={
                    fn ? (
                        <>
                            <p className="type-body-para text-fg">{fn.description}</p>
                            {!fn.enabled && (
                                <Notice tone="info" title="Switched off">
                                    {fn.disabled_by ? `${authorLabel(fn.disabled_by, agentName)} switched it off. ` : ''}The agent can’t call it until it is switched on.
                                </Notice>
                            )}
                            {nameTaken(fn.needs_attention) && (
                                <Notice tone="failed" title="Its name is taken">
                                    A tool attached after it has the name {fn.name}, so the agent can’t call this function until it is renamed.
                                </Notice>
                            )}
                            {attention.length > 0 && (
                                <Notice tone="failed" title="Needs attention">
                                    It calls {attention.join(', ')}, which the agent no longer has. A call fails at that step.
                                </Notice>
                            )}
                            <KeyValueList
                                items={[
                                    { label: 'Call as', value: <span className="type-mono-sm">{fn.call_as}(input)</span> },
                                    { label: 'Category', value: <Badge>{fn.category}</Badge> },
                                    { label: 'Made by', value: authorLabel(versions?.at(-1)?.author ?? fn.author, agentName) },
                                    ...(versions && versions.length > 1 ? [{ label: 'Last edited by', value: authorLabel(fn.author, agentName) }] : []),
                                    { label: 'Model-backed', value: fn.model_backed ? 'Yes' : 'No' },
                                    { label: 'Last called', value: fn.last_called_at ? relativeTime(fn.last_called_at) : 'Never' },
                                ]}
                            />
                            <CallList title="Calls" names={fn.calls} none="No other tools." />
                            <CallList title="Called by" names={fn.callers} none="No other function calls it." />
                            <section className="flex flex-col gap-8">
                                <Eyebrow as="h3" count={versions?.length || undefined}>
                                    History
                                </Eyebrow>
                                {versions === null ? (
                                    <Skeleton variant="lines" lines={3} />
                                ) : (
                                    <div className="-mx-8 flex flex-col">
                                        {versions.map((v) => (
                                            <RevisionRow
                                                key={v.version}
                                                revision={v.version}
                                                author={`${authorLabel(v.author, agentName)}${v.trigger === 'rollback' && v.restored_from ? ` · restored v${v.restored_from}` : ''}${v.reason ? ` · “${v.reason}”` : ''}`}
                                                time={relativeTime(v.at)}
                                                selected={compare === v.version}
                                                onSelect={v.version === fn.current_version ? undefined : () => pick(v.version)}
                                            />
                                        ))}
                                    </div>
                                )}
                            </section>
                        </>
                    ) : (
                        <Skeleton lines={5} />
                    )
                }
            >
                <div className="flex items-center gap-12">
                    <span className="flex-1 truncate type-mono-sm text-muted">
                        {view === 'script' ? `${fn?.name ?? name}.py` : view === 'input' ? 'input schema' : diff ? `v${diff.from} → v${diff.to}` : 'changes'}
                    </span>
                    {view === 'changes' && diff && canEdit && (
                        <Button size="sm" variant="secondary" icon="restore" onClick={() => void restore(diff.from)}>
                            Restore v{diff.from}
                        </Button>
                    )}
                    <FilterTabs
                        aria-label="Show"
                        value={view}
                        onChange={setView}
                        options={[
                            { value: 'script', label: 'Script' },
                            { value: 'input', label: 'Input' },
                            { value: 'changes', label: 'Changes' },
                        ]}
                    />
                </div>
                {!fn ? (
                    <Skeleton lines={14} />
                ) : view === 'script' ? (
                    <CodeBlock code={fn.script} tokens={scriptTokens} language="python" filename={`${fn.name}.py`} lineNumbers className="max-w-[100ch]" />
                ) : view === 'input' ? (
                    <div className="flex max-w-[100ch] flex-col gap-12">
                        <CodeBlock code={schemaText(fn.input_schema)} tokens={schemaTokens} language="json" filename="input.schema.json" />
                        {fn.output_description && (
                            <KeyValueList items={[{ label: 'Returns', value: fn.output_description }]} />
                        )}
                    </div>
                ) : diff ? (
                    <div className="flex max-w-[100ch] flex-col gap-14">
                        {diff.category && (
                            <p className="type-small text-muted">
                                Category <Badge>{diff.category.from}</Badge> → <Badge>{diff.category.to}</Badge>
                            </p>
                        )}
                        {diff.script.hunks.length > 0 && <DiffView variant="code" title="script" lines={diffLines(diff.script)} />}
                        {diff.description.hunks.length > 0 && <DiffView title="description" lines={diffLines(diff.description)} />}
                        {diff.input_schema.hunks.length > 0 && <DiffView variant="code" title="input schema" lines={diffLines(diff.input_schema)} />}
                        {!diff.category && diff.script.hunks.length + diff.description.hunks.length + diff.input_schema.hunks.length === 0 && (
                            <p className="type-small text-muted">Nothing the page shows changed between these versions.</p>
                        )}
                    </div>
                ) : (
                    <EmptyState
                        icon="restore"
                        title={versions && versions.length > 1 ? 'Pick a version' : 'No earlier versions'}
                        message={
                            versions && versions.length > 1
                                ? 'Choose an earlier version in History to see what changed since, and restore it.'
                                : 'Each edit is kept as a version. Once there is an earlier one, its changes show here.'
                        }
                    />
                )}
            </Panes>

            <ConfirmDialog
                open={deleting}
                title={`Delete ${fn?.call_as ?? 'function'}?`}
                removes={[
                    `The function and its ${fn?.current_version === 1 ? 'version' : `${fn?.current_version ?? ''} versions`}`,
                    ...(fn?.callers ?? []).map((c) => `${c} calls it and will need attention`),
                ]}
                message="The deletion stays in the agent’s activity."
                confirmLabel="Delete function"
                busy={deleteBusy}
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(false)}
            />
        </PageFrame>
    )
}

/** The functions or tools on one side of a call, as code names. */
function CallList({ title, names, none }: { title: string; names: string[]; none: string }) {
    return (
        <section className="flex flex-col gap-8">
            <Eyebrow as="h3" count={names.length || undefined}>
                {title}
            </Eyebrow>
            {names.length ? (
                <span className="flex flex-wrap gap-4">
                    {names.map((n) => (
                        <Badge key={n} className="font-mono">
                            {n}
                        </Badge>
                    ))}
                </span>
            ) : (
                <p className="type-small text-faint">{none}</p>
            )}
        </section>
    )
}
