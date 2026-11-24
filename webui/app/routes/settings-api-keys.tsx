import { Button, Dialog, IconButton, Notice, SecretReveal, Select, TextField } from '@vizier/design'
import { useState } from 'react'
import { createApiKey, deleteApiKey, type ApiKey } from '../api/endpoints/auth'
import { relativeTime } from '../lib/format'
import { usePagedList } from '../lib/usePagedList'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PagedTable } from '../screens/shared/PagedTable'
import { SettingsFrame } from '../screens/shared/SettingsFrame'
import { toast } from '../stores/toasts'

const SORTS = [
    { value: 'created', label: 'Newest' },
    { value: 'last_used', label: 'Last used' },
]

const EXPIRY = [
    { value: 'never', label: 'Never expires' },
    { value: '30', label: '30 days' },
    { value: '90', label: '90 days' },
    { value: '365', label: '1 year' },
]

export default function SettingsApiKeys() {
    const list = usePagedList<ApiKey>('/auth/api-keys', { sort: 'created' })
    const [creating, setCreating] = useState(false)
    const [revealed, setRevealed] = useState<{ name: string; key: string }>()
    const [deleting, setDeleting] = useState<ApiKey | null>(null)

    return (
        <SettingsFrame
            width="wide"
            title="API keys"
            description="Keys let scripts and other programs talk to Vizier as you. A key is shown once, when it is made."
            actions={
                <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>
                    New key
                </Button>
            }
        >
            {revealed && (
                <SecretReveal title={`Key “${revealed.name}” created. Copy it now: it won’t be shown again.`} secret={revealed.key} />
            )}
            <PagedTable<ApiKey>
                label="API keys"
                searchLabel="Search keys"
                list={list}
                rowKey={(k) => k.id}
                sorts={SORTS}
                columns={[{ label: 'Name' }, { label: 'Prefix', width: 160 }, { label: 'Last used', width: 160 }, { label: '', width: 56, align: 'end' }]}
                endAligned={[3]}
                row={(k) => [
                    <span key="n" className="type-body-strong text-fg">{k.name}</span>,
                    <span key="p" className="type-mono-xs text-muted">{k.prefix ? `${k.prefix}…` : '–'}</span>,
                    <span key="l" className="type-small text-muted">{k.last_used_at ? relativeTime(k.last_used_at) : 'never'}</span>,
                    <IconButton key="d" icon="trash" aria-label={`Delete ${k.name}`} onClick={() => setDeleting(k)} />,
                ]}
                empty={{
                    icon: 'key',
                    title: 'No API keys',
                    message: 'Make a key to call the Vizier API from a script.',
                    action: (
                        <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>
                            New key
                        </Button>
                    ),
                }}
            />

            <Dialog open={creating} title="New API key" confirmLabel="" onConfirm={() => {}} onCancel={() => setCreating(false)}>
                {creating && (
                    <NewKeyForm
                        onCancel={() => setCreating(false)}
                        onCreated={(name, key) => {
                            setCreating(false)
                            setRevealed({ name, key })
                            list.reload()
                        }}
                    />
                )}
            </Dialog>

            <ConfirmDialog
                open={deleting !== null}
                title={`Delete ${deleting?.name ?? 'key'}?`}
                message="Anything using this key stops working at once."
                confirmLabel="Delete key"
                onConfirm={async () => {
                    const key = deleting
                    setDeleting(null)
                    if (!key) return
                    try {
                        await deleteApiKey(key.id)
                        list.reload()
                        toast.success('Key deleted')
                    } catch (e) {
                        toast.error('Couldn’t delete the key', e instanceof Error ? e.message : undefined)
                    }
                }}
                onCancel={() => setDeleting(null)}
            />
        </SettingsFrame>
    )
}

function NewKeyForm({ onCancel, onCreated }: { onCancel: () => void; onCreated: (name: string, key: string) => void }) {
    const [name, setName] = useState('')
    const [expiry, setExpiry] = useState('never')
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                try {
                    const made = await createApiKey(name.trim(), expiry === 'never' ? undefined : Number(expiry))
                    onCreated(made.name, made.key)
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'The key could not be made.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {error && <Notice tone="failed" title="The key wasn’t made">{error}</Notice>}
            <TextField label="Name" value={name} placeholder="home-assistant" helper="What the key is for." autoFocus onChange={(e) => setName(e.target.value)} />
            <Select label="Expires" options={EXPIRY} value={expiry} onChange={setExpiry} />
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>Cancel</Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!name.trim()}>Create key</Button>
            </div>
        </form>
    )
}
