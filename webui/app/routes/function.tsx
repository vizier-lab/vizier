import {
    Badge,
    Button,
    CodeBlock,
    Dialog,
    DiffView,
    EmptyState,
    KeyValueList,
    Notice,
    RevisionRow,
    Select,
    Skeleton,
    Switch,
    Tab,
    TabList,
    Textarea,
    TextField,
} from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    deleteFunction,
    diffFunction,
    editFunction,
    getFunction,
    listFunctions,
    listFunctionVersions,
    rollbackFunction,
    setFunctionEnabled,
    type FunctionDetail,
    type FunctionDiff,
    type FunctionEdit,
    type FunctionProblem,
    type FunctionVersion,
} from '../api/endpoints/functions'
import { relativeTime } from '../lib/format'
import {
    NAME_PATTERN,
    authorLabel,
    isOutOfDate,
    missingCalls,
    offNote,
    parseSchema,
    problemText,
    problemsOf,
    schemaText,
} from '../lib/functions'
import { diffLines } from '../lib/revisions'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'

const messageOf = (e: unknown, fallback: string) => (e instanceof Error && e.message ? e.message : fallback)

type View = 'script' | 'input' | 'changes'

/** One function (spec 061): its script and input, what it calls, its history, and for people who
 * can edit the agent: edit, switch, roll back and delete. */
export default function FunctionPage() {
    const { agentId = '', name = '' } = useParams()
    const navigate = useNavigate()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const { canEditAgent } = useSession()
    const canEdit = agent ? canEditAgent(agent) : false
    const agentName = agent?.name
    const [fn, setFn] = useState<FunctionDetail | null>(null)
    const [missing, setMissing] = useState(false)
    const [versions, setVersions] = useState<FunctionVersion[]>([])
    const [view, setView] = useState<View>('script')
    const [compare, setCompare] = useState<number | null>(null)
    const [diff, setDiff] = useState<FunctionDiff | null>(null)
    const [editing, setEditing] = useState(false)
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
            toast.success(`${fn.call_as} deleted`)
            navigate(`/${agentId}/functions`)
        } catch (e) {
            toast.error('Couldn’t delete it', messageOf(e, ''))
        } finally {
            setDeleteBusy(false)
            setDeleting(false)
        }
    }

    const back = { href: `/${agentId}/functions`, label: 'Functions' }
    if (missing) {
        return (
            <PageFrame title={name} back={back}>
                <EmptyState kind="error" title="This function isn’t there" message="It may have been deleted or renamed. Every function is listed on Functions." />
            </PageFrame>
        )
    }

    const attention = missingCalls(fn?.needs_attention)
    return (
        <PageFrame
            fill
            back={back}
            title={fn?.call_as ?? name}
            meta={
                fn
                    ? `${fn.category} · version ${fn.current_version} · ${authorLabel(fn.author, agentName)} · ${relativeTime(fn.updated_at)}${fn.enabled ? '' : ` · ${offNote(fn.disabled_by, agentName)}`}`
                    : undefined
            }
            actions={
                canEdit && (
                    <>
                        {fn && <Switch checked={fn.enabled} onCheckedChange={(on) => void toggle(on)} aria-label={fn.enabled ? 'Switch off' : 'Switch on'} />}
                        <Button variant="ghost" icon="trash" disabled={!fn} onClick={() => setDeleting(true)}>
                            Delete
                        </Button>
                        <Button variant="primary" icon="pencil" disabled={!fn} onClick={() => setEditing(true)}>
                            Edit
                        </Button>
                    </>
                )
            }
        >
            <Panes
                paneLabel="About and history"
                pane={
                    <div className="flex flex-col gap-20 p-16">
                        {fn ? (
                            <>
                                {attention.length > 0 && (
                                    <Notice tone="failed" title="Needs attention">
                                        It calls {attention.join(', ')}, which the agent no longer has. A call fails at that step.
                                    </Notice>
                                )}
                                <KeyValueList
                                    items={[
                                        { label: 'Calls', value: fn.calls.length ? fn.calls.join(', ') : 'no tools' },
                                        { label: 'Called by', value: fn.callers.length ? fn.callers.map((c) => `custom_${c}`).join(', ') : 'no other function' },
                                        { label: 'Model-backed', value: fn.model_backed ? 'yes' : 'no' },
                                        { label: 'Last called', value: fn.last_called_at ? relativeTime(fn.last_called_at) : 'never' },
                                    ]}
                                />
                                <div className="flex flex-col">
                                    <p className="pb-6 type-ui-strong text-fg">History</p>
                                    {versions.map((v) => (
                                        <RevisionRow
                                            key={v.version}
                                            revision={v.version}
                                            author={`${authorLabel(v.author, agentName)}${v.trigger === 'rollback' && v.restored_from ? ` · restored v${v.restored_from}` : ''}${v.reason ? ` · “${v.reason}”` : ''}`}
                                            time={relativeTime(v.at)}
                                            selected={compare === v.version}
                                            onSelect={
                                                v.version === fn.current_version
                                                    ? undefined
                                                    : () => {
                                                          setCompare(v.version)
                                                          setView('changes')
                                                      }
                                            }
                                        />
                                    ))}
                                </div>
                            </>
                        ) : (
                            <Skeleton className="h-160" />
                        )}
                    </div>
                }
            >
                <div className="flex min-h-0 flex-1 flex-col gap-14 overflow-auto p-16 sm:p-24">
                    {fn && <p className="type-body text-fg">{fn.description}</p>}
                    <TabList aria-label="Function">
                        <Tab selected={view === 'script'} onClick={() => setView('script')}>
                            Script
                        </Tab>
                        <Tab selected={view === 'input'} onClick={() => setView('input')}>
                            Input
                        </Tab>
                        <Tab selected={view === 'changes'} onClick={() => setView('changes')}>
                            Changes
                        </Tab>
                    </TabList>
                    {!fn ? (
                        <Skeleton className="h-240" />
                    ) : view === 'script' ? (
                        <CodeBlock code={fn.script} language="python" lineNumbers />
                    ) : view === 'input' ? (
                        <div className="flex flex-col gap-10">
                            <CodeBlock code={schemaText(fn.input_schema)} language="json" />
                            {fn.output_description && <p className="type-small-para text-muted">Returns: {fn.output_description}</p>}
                        </div>
                    ) : diff ? (
                        <div className="flex flex-col gap-14">
                            {diff.category && (
                                <p className="type-small text-muted">
                                    Category <Badge>{diff.category.from}</Badge> → <Badge>{diff.category.to}</Badge>
                                </p>
                            )}
                            <DiffView
                                variant="code"
                                title={`script · v${diff.from} → v${diff.to}`}
                                lines={diffLines(diff.script)}
                                restoreLabel={canEdit ? `Restore v${diff.from}` : undefined}
                                onRestore={canEdit ? () => void restore(diff.from) : undefined}
                            />
                            {diff.description.hunks.length > 0 && <DiffView title="description" lines={diffLines(diff.description)} />}
                            {diff.input_schema.hunks.length > 0 && <DiffView variant="code" title="input schema" lines={diffLines(diff.input_schema)} />}
                        </div>
                    ) : (
                        <EmptyState icon="restore" title="Pick a version" message="Choose an earlier version in History to see what changed since, and restore it." />
                    )}
                </div>
            </Panes>

            <Dialog open={editing} title={`Edit ${fn?.call_as ?? ''}`} confirmLabel="" onConfirm={() => {}} onCancel={() => setEditing(false)}>
                {editing && fn && (
                    <EditForm
                        agentId={agentId}
                        fn={fn}
                        onCancel={() => setEditing(false)}
                        onReload={() => {
                            setEditing(false)
                            load()
                        }}
                        onSaved={(version, newName) => {
                            setEditing(false)
                            toast.success(`Saved as version ${version}`)
                            if (newName !== fn.name) navigate(`/${agentId}/functions/${encodeURIComponent(newName)}`, { replace: true })
                            else load()
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting}
                title={`Delete ${fn?.call_as ?? ''}?`}
                message={`Its ${fn?.current_version ?? ''} version(s) are deleted. The deletion stays in the agent’s activity.`}
                removes={fn?.callers.length ? fn.callers.map((c) => `custom_${c} calls it and will need attention`) : undefined}
                confirmLabel="Delete function"
                busy={deleteBusy}
                onConfirm={() => void remove()}
                onCancel={() => setDeleting(false)}
            />
        </PageFrame>
    )
}

const NEW_CATEGORY = '__new__'

type EditFormProps = {
    agentId: string
    fn: FunctionDetail
    onCancel: () => void
    onReload: () => void
    onSaved: (version: number, name: string) => void
}

/** A person's edit: saved as a new version they author. No sample run; every problem at once. */
function EditForm({ agentId, fn, onCancel, onReload, onSaved }: EditFormProps) {
    const [name, setName] = useState(fn.name)
    const [description, setDescription] = useState(fn.description)
    const [categories, setCategories] = useState<string[]>([fn.category])
    const [category, setCategory] = useState(fn.category)
    const [newCategory, setNewCategory] = useState('')
    const [newCategoryDescription, setNewCategoryDescription] = useState('')
    const [schema, setSchema] = useState(schemaText(fn.input_schema))
    const [output, setOutput] = useState(fn.output_description ?? '')
    const [script, setScript] = useState(fn.script)
    const [reason, setReason] = useState('')
    const [busy, setBusy] = useState(false)
    const [problems, setProblems] = useState<FunctionProblem[]>([])
    const [stale, setStale] = useState(false)

    useEffect(() => {
        listFunctions(agentId).then(
            (l) => setCategories(l.categories.map((c) => c.name)),
            () => {}
        )
    }, [agentId])

    const creating = category === NEW_CATEGORY
    const nameOk = NAME_PATTERN.test(name)
    const newCategoryOk = !creating || (NAME_PATTERN.test(newCategory) && newCategoryDescription.trim().length > 0)
    const canSave = nameOk && newCategoryOk && description.trim().length > 0 && !busy

    const submit = async () => {
        const parsed = parseSchema(schema)
        if (!parsed.ok) {
            setProblems([{ code: 'schema_json', message: parsed.message }])
            return
        }
        const targetCategory = creating ? newCategory : category
        const edit: FunctionEdit = {
            expected_version: fn.current_version,
            name: name !== fn.name ? name : undefined,
            description: description.trim() !== fn.description ? description.trim() : undefined,
            category: targetCategory !== fn.category ? targetCategory : undefined,
            category_description: creating ? newCategoryDescription.trim() : undefined,
            input_schema: schema !== schemaText(fn.input_schema) ? parsed.value : undefined,
            output_description: output !== (fn.output_description ?? '') ? output : undefined,
            script: script !== fn.script ? script : undefined,
            reason: reason.trim() || undefined,
        }
        setBusy(true)
        setProblems([])
        try {
            const saved = await editFunction(agentId, fn.id, edit)
            onSaved(saved.version, saved.tool?.name ?? name)
        } catch (e) {
            if (isOutOfDate(e)) setStale(true)
            else setProblems(problemsOf(e))
        } finally {
            setBusy(false)
        }
    }

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={(e) => {
                e.preventDefault()
                if (canSave) void submit()
            }}
        >
            {stale && (
                <Notice tone="failed" title="This function changed since you opened it" actions={<Button onClick={onReload}>Reload</Button>}>
                    Reload to see the change, then edit again.
                </Notice>
            )}
            {problems.length > 0 && (
                <Notice tone="failed" title="Not saved, fix these">
                    <ul className="list-disc pl-20">
                        {problems.map((p, i) => (
                            <li key={i}>{problemText(p)}</li>
                        ))}
                    </ul>
                </Notice>
            )}
            <TextField
                label="Name"
                mono
                value={name}
                autoComplete="off"
                spellCheck={false}
                helper={nameOk ? `The agent calls it as custom_${name}.` : 'Lower-case letters, digits and _, starting with a letter.'}
                invalid={!nameOk}
                onChange={(e) => setName(e.target.value)}
            />
            <TextField label="Description" value={description} onChange={(e) => setDescription(e.target.value)} />
            <Select
                label="Category"
                value={category}
                onChange={setCategory}
                options={[...categories.map((c) => ({ value: c, label: c })), { value: NEW_CATEGORY, label: 'New category…' }]}
            />
            {creating && (
                <>
                    <TextField label="New category name" mono value={newCategory} invalid={newCategory.length > 0 && !NAME_PATTERN.test(newCategory)} onChange={(e) => setNewCategory(e.target.value)} />
                    <TextField
                        label="New category description"
                        value={newCategoryDescription}
                        helper="Required for a new category: one line saying what it is for."
                        onChange={(e) => setNewCategoryDescription(e.target.value)}
                    />
                </>
            )}
            <Textarea label="Input schema (JSON)" mono rows={6} value={schema} spellCheck={false} onChange={(e) => setSchema(e.target.value)} />
            <TextField label="What it returns" value={output} helper="Optional." onChange={(e) => setOutput(e.target.value)} />
            <Textarea
                label="Script"
                mono
                rows={12}
                value={script}
                spellCheck={false}
                helper="The arguments are the dict input; the last expression is the result."
                onChange={(e) => setScript(e.target.value)}
            />
            <TextField label="Why" value={reason} helper="Optional, kept with the version." onChange={(e) => setReason(e.target.value)} />
            <div className="flex justify-end gap-10">
                <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!canSave}>
                    Save as v{fn.current_version + 1}
                </Button>
            </div>
        </form>
    )
}
