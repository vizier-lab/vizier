import { Button, Dialog, EmptyState, FunctionGroup, FunctionRow, Notice, Skeleton, TextField } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    listFunctions,
    setFunctionEnabled,
    updateFunctionCategory,
    type FunctionCategory,
    type FunctionList,
    type FunctionSummary,
} from '../api/endpoints/functions'
import { relativeTime } from '../lib/format'
import { NAME_PATTERN, authorLabel, missingCalls, offNote } from '../lib/functions'
import { PageFrame } from '../screens/shared/PageFrame'
import { useAgents } from '../stores/agents'
import { useSession } from '../stores/session'
import { toast } from '../stores/toasts'

const messageOf = (e: unknown, fallback: string) => (e instanceof Error && e.message ? e.message : fallback)

/**
 * An agent's functions (spec 061): the tools it saved from its own scripts, grouped by category.
 * The agent makes them; people who can edit the agent switch them off, rename categories, and
 * edit, roll back or delete one from its own page.
 */
export default function Functions() {
    const { agentId = '' } = useParams()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const { canEditAgent } = useSession()
    const canEdit = agent ? canEditAgent(agent) : false
    const agentName = agent?.name
    const [list, setList] = useState<FunctionList | null>(null)
    const [error, setError] = useState<unknown>()
    const [editing, setEditing] = useState<FunctionCategory | null>(null)

    const load = useCallback(() => {
        listFunctions(agentId).then(
            (l) => {
                setList(l)
                setError(undefined)
            },
            setError
        )
    }, [agentId])

    useEffect(load, [load])

    const toggle = async (fn: FunctionSummary, enabled: boolean) => {
        try {
            await setFunctionEnabled(agentId, fn.id, enabled)
            toast.success(`${fn.call_as} switched ${enabled ? 'on' : 'off'}`)
            load()
        } catch (e) {
            toast.error(`Couldn’t switch ${fn.call_as} ${enabled ? 'on' : 'off'}`, messageOf(e, ''))
        }
    }

    const notFound = error instanceof ApiError && error.status === 404
    const count = list?.categories.reduce((n, c) => n + c.tools.length, 0) ?? 0

    return (
        <PageFrame
            title="Functions"
            meta="Tools this agent made from its own scripts. It creates them; you can switch them off, edit, roll back or delete them."
            width="wide"
        >
            {notFound ? (
                <EmptyState kind="error" title="This agent isn’t there" message="It may have been deleted, or you can’t see it." />
            ) : error ? (
                <EmptyState
                    kind="error"
                    title="Couldn’t load the functions"
                    message={messageOf(error, '')}
                    action={<Button onClick={load}>Retry</Button>}
                />
            ) : !list ? (
                <div className="flex flex-col gap-14">
                    <Skeleton className="h-120" />
                    <Skeleton className="h-80" />
                </div>
            ) : count === 0 ? (
                <EmptyState
                    icon="code"
                    title="No functions yet"
                    message="The agent makes a function when it saves a workflow it repeats, from a conversation or a dream. They show up here."
                />
            ) : (
                <div className="flex flex-col gap-14">
                    {!list.tool_names_known && (
                        <Notice tone="info" title="Some checks are unavailable">
                            The agent hasn’t run since it started, so missing tools can’t be shown yet.
                        </Notice>
                    )}
                    {list.categories.map((category) => (
                        <FunctionGroup
                            key={category.name}
                            name={category.name}
                            description={category.description}
                            onEdit={canEdit ? () => setEditing(category) : undefined}
                        >
                            {category.tools.map((fn) => (
                                <FunctionRow
                                    key={fn.id}
                                    callAs={fn.call_as}
                                    description={fn.description}
                                    version={fn.version}
                                    meta={`${authorLabel(fn.author, agentName)} · ${relativeTime(fn.updated_at)}`}
                                    enabled={fn.enabled}
                                    offNote={offNote(fn.disabled_by, agentName)}
                                    modelBacked={fn.model_backed}
                                    missing={missingCalls(fn.needs_attention)}
                                    onEnabledChange={canEdit ? (on) => void toggle(fn, on) : undefined}
                                    href={`/${agentId}/functions/${encodeURIComponent(fn.name)}`}
                                />
                            ))}
                        </FunctionGroup>
                    ))}
                </div>
            )}

            <Dialog open={editing !== null} title={`Category ${editing?.name ?? ''}`} confirmLabel="" onConfirm={() => {}} onCancel={() => setEditing(null)}>
                {editing && (
                    <CategoryForm
                        category={editing}
                        onCancel={() => setEditing(null)}
                        onSave={async (change) => {
                            await updateFunctionCategory(agentId, editing.name, change)
                            setEditing(null)
                            toast.success('Category saved', 'The agent sees the change from its next turn.')
                            load()
                        }}
                    />
                )}
            </Dialog>
        </PageFrame>
    )
}

type CategoryFormProps = {
    category: FunctionCategory
    onCancel: () => void
    onSave: (change: { description?: string; new_name?: string }) => Promise<void>
}

/** Rename or redescribe a category. Functions keep their names. */
function CategoryForm({ category, onCancel, onSave }: CategoryFormProps) {
    const [name, setName] = useState(category.name)
    const [description, setDescription] = useState(category.description)
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()
    const nameOk = NAME_PATTERN.test(name)
    const changed = name !== category.name || description.trim() !== category.description
    const canSave = nameOk && description.trim().length > 0 && changed && !busy

    const submit = async () => {
        setBusy(true)
        setProblem(undefined)
        try {
            await onSave({
                new_name: name !== category.name ? name : undefined,
                description: description.trim() !== category.description ? description.trim() : undefined,
            })
        } catch (e) {
            setProblem(e instanceof ApiError && e.status === 409 ? `There is already a category named ${name}.` : messageOf(e, 'It wasn’t saved.'))
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
            {problem && <Notice tone="failed" title="It wasn’t saved">{problem}</Notice>}
            <TextField
                label="Name"
                mono
                value={name}
                autoComplete="off"
                spellCheck={false}
                helper={nameOk ? 'Functions keep their names when their category is renamed.' : 'Lower-case letters, digits and _, starting with a letter.'}
                invalid={!nameOk}
                onChange={(e) => setName(e.target.value)}
            />
            <TextField label="Description" value={description} helper="One line saying what the category is for." onChange={(e) => setDescription(e.target.value)} />
            <p className="type-small-para text-muted">The agent sees the change from its next turn.</p>
            <div className="flex justify-end gap-10">
                <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!canSave}>
                    Save
                </Button>
            </div>
        </form>
    )
}
