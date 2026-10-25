import { Button, Dialog, EmptyState, Notice, SecretField, TextField } from '@vizier/design'
import { useState } from 'react'
import { useParams } from 'react-router'
import { ApiError } from '../api/client'
import { createSecret, deleteSecret, replaceSecret, type SecretItem } from '../api/endpoints/secrets'
import { relativeTime } from '../lib/format'
import { usePagedList } from '../lib/usePagedList'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PagedTable } from '../screens/shared/PagedTable'
import { PageFrame } from '../screens/shared/PageFrame'
import { Panes } from '../screens/shared/Panes'
import { ActivityCard } from '../screens/secrets/ActivityCard'
import { toast } from '../stores/toasts'

const NAME_RULE = "1–128 characters: letters, digits, '.', '_' or '-'."
const VALUE_RULE = '8–65,536 bytes. You won’t be able to view it again.'
const NAME_PATTERN = /^[A-Za-z0-9._-]{1,128}$/
const MIN_VALUE_BYTES = 8
const MAX_VALUE_BYTES = 65_536

const SORTS = [
    { value: 'name', label: 'Name' },
    { value: 'updated', label: 'Recently updated' },
]

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
    const list = usePagedList<SecretItem>(`/agents/${agentId}/secrets`, { sort: 'name' })
    const [version, setVersion] = useState(0)
    const [creating, setCreating] = useState(false)
    const [replacing, setReplacing] = useState<SecretItem | null>(null)
    const [deleting, setDeleting] = useState<SecretItem | null>(null)
    const [deleteBusy, setDeleteBusy] = useState(false)
    const [deleteError, setDeleteError] = useState<string>()

    const reload = () => {
        list.reload()
        setVersion((v) => v + 1)
    }

    const forbidden = list.error instanceof ApiError && list.error.status === 403

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
            fill={!forbidden}
            title="Secrets"
            meta="Credentials this agent can use without seeing them · values are masked after saving"
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
            ) : (
                <Panes paneLabel="Recent changes" pane={<ActivityCard agentId={agentId} version={version} />}>
                    <PagedTable<SecretItem>
                        label="Secrets"
                        searchLabel="Search secrets"
                        list={list}
                        rowKey={(secret) => secret.name}
                        sorts={SORTS}
                        columns={[
                            { label: 'Secret' },
                            { label: 'Value', width: 240 },
                            { label: '', width: 120, align: 'end', srLabel: 'Actions' },
                        ]}
                        endAligned={[2]}
                        row={(secret) => [
                            <span key="n" className="flex min-w-0 flex-col">
                                <span className="truncate type-code text-fg">{secret.name}</span>
                                <span className="truncate type-small text-faint">{changeText(secret)}</span>
                            </span>,
                            <SecretField key="v" masked={secret.masked} onReplace={() => setReplacing(secret)} />,
                            <Button key="d" variant="ghost" icon="trash" aria-label={`Delete ${secret.name}`} onClick={() => setDeleting(secret)}>
                                Delete
                            </Button>,
                        ]}
                        empty={{
                            icon: 'key',
                            title: 'No secrets yet',
                            message: 'Store a credential this agent needs. It can read it from a script; neither you nor the model can see it again.',
                            action: (
                                <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>
                                    Add secret
                                </Button>
                            ),
                        }}
                    />
                </Panes>
            )}

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
