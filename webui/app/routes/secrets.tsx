import { ActivityItem, Button, Dialog, EmptyState, Notice, PanelHeader, SecretField, Skeleton, TextField } from '@vizier/design'
import { useCallback, useEffect, useState } from 'react'
import { useParams } from 'react-router'
import { ApiError } from '../api/client'
import {
    createSecret,
    deleteSecret,
    listSecretEvents,
    listSecrets,
    replaceSecret,
    type SecretEvent,
    type SecretItem,
} from '../api/endpoints/secrets'
import { absoluteTime, relativeTime } from '../lib/format'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const NAME_RULE = "1–128 characters: letters, digits, '.', '_' or '-'."
const VALUE_RULE = '8–65,536 bytes. You won’t be able to view it again.'
const NAME_PATTERN = /^[A-Za-z0-9._-]{1,128}$/
const MIN_VALUE_BYTES = 8
const MAX_VALUE_BYTES = 65_536

const messageOf = (e: unknown, fallback: string) => (e instanceof Error && e.message ? e.message : fallback)

/** Bytes, not characters: the server counts the stored UTF-8 exactly. */
const byteLength = (value: string) => new TextEncoder().encode(value).length

/** The list's wording for a secret's last change: "created by alice just now". */
function changeText(secret: SecretItem): string {
    const action = new Date(secret.updated_at).getTime() === new Date(secret.created_at).getTime() ? 'created' : 'replaced'
    return `${action} by ${secret.updated_by_name} ${relativeTime(secret.updated_at)}`
}

/**
 * An agent's secrets (027): names, masks and who changed them. No screen ever holds a
 * value — create and replace send one and forget it, and nothing here can reveal it.
 */
export default function Secrets() {
    const { agentId = '' } = useParams()
    const [items, setItems] = useState<SecretItem[] | null>(null)
    const [error, setError] = useState<unknown>(null)
    const [events, setEvents] = useState<SecretEvent[] | null>(null)
    const [eventsError, setEventsError] = useState<string>()
    const [version, setVersion] = useState(0)
    const [creating, setCreating] = useState(false)
    const [replacing, setReplacing] = useState<SecretItem | null>(null)
    const [deleting, setDeleting] = useState<SecretItem | null>(null)
    const [deleteBusy, setDeleteBusy] = useState(false)
    const [deleteError, setDeleteError] = useState<string>()

    const reload = useCallback(() => setVersion((v) => v + 1), [])

    // A different agent is a different store: drop the old rows before fetching.
    useEffect(() => {
        setItems(null)
        setEvents(null)
        setError(null)
        setEventsError(undefined)
    }, [agentId])

    useEffect(() => {
        let live = true
        setError(null)
        listSecrets(agentId)
            .then((list) => live && setItems(list.items))
            .catch((e) => live && setError(e))
        return () => {
            live = false
        }
    }, [agentId, version])

    useEffect(() => {
        let live = true
        setEventsError(undefined)
        listSecretEvents(agentId, 20)
            .then((list) => live && setEvents(list.items))
            .catch((e) => live && setEventsError(messageOf(e, 'The change log could not be loaded.')))
        return () => {
            live = false
        }
    }, [agentId, version])

    const forbidden = error instanceof ApiError && error.status === 403
    const failed = error !== null && !forbidden

    const cancelDelete = () => {
        if (deleteBusy) return
        setDeleting(null)
        setDeleteError(undefined)
    }

    const confirmDelete = async () => {
        const target = deleting
        if (!target) return
        setDeleteBusy(true)
        setDeleteError(undefined)
        try {
            await deleteSecret(agentId, target.name)
            setDeleting(null)
            toast.success(`Secret “${target.name}” deleted`)
            reload()
        } catch (e) {
            setDeleteError(messageOf(e, 'The secret could not be deleted.'))
        } finally {
            setDeleteBusy(false)
        }
    }

    return (
        <PageFrame
            title="Secrets"
            description="Credentials this agent can use without seeing them. Values are masked after saving."
            actions={
                !forbidden && (
                    <Button variant="primary" icon="plus" disabled={creating} onClick={() => setCreating(true)}>
                        Add secret
                    </Button>
                )
            }
        >
            {forbidden ? (
                <EmptyState
                    kind="error"
                    icon="lock"
                    title="You can’t manage this agent’s secrets"
                    message="Secrets require the “Manage every agent’s secrets” or “Manage secrets of agents they own” permission."
                />
            ) : failed ? (
                <EmptyState kind="error" title="Couldn’t load secrets" message={messageOf(error, 'The server could not list them.')} />
            ) : !items ? (
                <div className="flex flex-col gap-8" aria-busy>
                    <Skeleton className="h-56" />
                    <Skeleton className="h-56" />
                    <Skeleton className="h-56" />
                </div>
            ) : items.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="key"
                    title="No secrets yet"
                    message="Store a credential this agent needs. It can read it from a script; neither you nor the model can see it again."
                    action={
                        <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>
                            Add secret
                        </Button>
                    }
                />
            ) : (
                <ul aria-label="Secrets" className="flex flex-col rounded-lg border border-border">
                    {items.map((secret) => (
                        <li
                            key={secret.name}
                            className="flex flex-col gap-10 border-b border-border px-16 py-14 last:border-b-0 sm:flex-row sm:items-center sm:justify-between"
                        >
                            <div className="flex min-w-0 flex-col gap-4">
                                <span className="type-code text-fg">{secret.name}</span>
                                <span className="type-small text-faint">
                                    <time dateTime={secret.updated_at} title={absoluteTime(secret.updated_at)}>
                                        {changeText(secret)}
                                    </time>
                                </span>
                            </div>
                            <div className="flex shrink-0 items-center gap-16">
                                <SecretField masked={secret.masked} onReplace={() => setReplacing(secret)} />
                                <Button variant="ghost" icon="trash" onClick={() => setDeleting(secret)}>
                                    Delete
                                </Button>
                            </div>
                        </li>
                    ))}
                </ul>
            )}

            <section aria-label="Recent changes" className="flex flex-col gap-8">
                <PanelHeader title="Recent changes" level={2} />
                {eventsError ? (
                    <p className="type-small text-failed-text" role="alert">
                        {eventsError}
                    </p>
                ) : !events ? (
                    <Skeleton variant="lines" lines={2} />
                ) : events.length === 0 ? (
                    <p className="type-small text-faint">Nothing has changed yet. Every save and deletion is recorded here, with who made it.</p>
                ) : (
                    <div className="flex flex-col">
                        {events.map((event) => (
                            <ActivityItem
                                key={event.id}
                                actor={event.actor_name}
                                time={
                                    <time dateTime={event.at} title={absoluteTime(event.at)}>
                                        {relativeTime(event.at)}
                                    </time>
                                }
                                text={
                                    <>
                                        {event.action} <span className="type-code">{event.name}</span>
                                    </>
                                }
                            />
                        ))}
                    </div>
                )}
            </section>

            <Dialog open={creating} title="Add secret" confirmLabel="" onConfirm={() => {}} onCancel={() => setCreating(false)}>
                {creating && (
                    <SecretForm
                        mode="create"
                        onCancel={() => setCreating(false)}
                        onSave={async (name, value) => {
                            const write = await createSecret(agentId, name, value)
                            setCreating(false)
                            toast.success(`Secret “${write.secret.name}” ${write.status === 'created' ? 'created' : 'replaced'}`)
                            reload()
                        }}
                    />
                )}
            </Dialog>

            <Dialog open={replacing !== null} title="Replace secret" confirmLabel="" onConfirm={() => {}} onCancel={() => setReplacing(null)}>
                {replacing && (
                    <SecretForm
                        mode="replace"
                        name={replacing.name}
                        message="The next retrieval sees the new value. The agent is not restarted and a running turn is not interrupted."
                        onCancel={() => setReplacing(null)}
                        onSave={async (name, value) => {
                            await replaceSecret(agentId, name, value)
                            setReplacing(null)
                            toast.success(`Secret “${name}” replaced`)
                            reload()
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete “${deleting?.name ?? ''}”?`}
                message={`The agent’s next retrieval of this name returns not_found. This cannot be undone.`}
                confirmLabel="Delete secret"
                busy={deleteBusy}
                error={deleteError}
                onConfirm={() => void confirmDelete()}
                onCancel={cancelDelete}
            />
        </PageFrame>
    )
}

type FormProps = {
    mode: 'create' | 'replace'
    /** Fixed in replace mode. */
    name?: string
    message?: string
    onCancel: () => void
    onSave: (name: string, value: string) => Promise<void>
}

/** The create and replace forms differ only in the name field and the sentence above it. */
function SecretForm({ mode, name: fixedName, message, onCancel, onSave }: FormProps) {
    const [name, setName] = useState(fixedName ?? '')
    const [value, setValue] = useState('')
    const [busy, setBusy] = useState(false)
    const [problem, setProblem] = useState<string>()
    const [problemOn, setProblemOn] = useState<'name' | 'value'>('value')

    const nameOk = NAME_PATTERN.test(name)
    const valueOk = byteLength(value) >= MIN_VALUE_BYTES && byteLength(value) <= MAX_VALUE_BYTES
    const canSave = nameOk && valueOk && !busy

    const submit = async () => {
        setBusy(true)
        setProblem(undefined)
        try {
            // The value is sent exactly as typed: the server stores it without trimming.
            await onSave(mode === 'replace' ? (fixedName ?? name) : name, value)
        } catch (e) {
            const text = messageOf(e, 'The secret could not be saved.')
            setProblem(text)
            // The server states which rule failed; put that under the field it names.
            setProblemOn(text.toLowerCase().includes('name') ? 'name' : 'value')
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
            {message && <p className="type-small-para text-muted">{message}</p>}
            {problem && <Notice tone="failed" title="It wasn’t saved">{problem}</Notice>}
            <TextField
                label="Name"
                mono
                locked={mode === 'replace'}
                readOnly={mode === 'replace'}
                value={name}
                autoComplete="off"
                spellCheck={false}
                helper={problem && problemOn === 'name' ? problem : NAME_RULE}
                invalid={!!problem && problemOn === 'name'}
                autoFocus={mode === 'create'}
                onChange={(e) => setName(e.target.value)}
            />
            <TextField
                label="Value"
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={value}
                helper={problem && problemOn === 'value' ? problem : VALUE_RULE}
                invalid={!!problem && problemOn === 'value'}
                autoFocus={mode === 'replace'}
                onChange={(e) => setValue(e.target.value)}
            />
            <div className="flex justify-end gap-10">
                <Button type="button" variant="ghost" onClick={onCancel} disabled={busy}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!canSave}>
                    {mode === 'create' ? 'Add secret' : 'Replace value'}
                </Button>
            </div>
        </form>
    )
}
