import { Button, Notice, TextField } from '@vizier/design'
import { useState } from 'react'
import type { ProviderSummary } from '../../api/endpoints/catalogue'
import { saveProvider, testProvider, type CredentialField, type TestResult } from '../../api/endpoints/providers'
import { FIELD_LABELS, PLAIN_FIELDS } from '../../lib/providers'

type Props = {
    provider: Pick<ProviderSummary, 'id' | 'name' | 'fields' | 'endpoint' | 'doc'>
    /** Editing a connected provider: secrets left empty are kept. */
    editing?: boolean
    onDone: () => void
    onCancel: () => void
    submitLabel?: string
}

/**
 * Connect one provider with the fields it takes (a key, an address, a token). Saving also checks
 * the connection, and says what came of it: the credentials are kept either way.
 */
export function ConnectForm({ provider, editing, onDone, onCancel, submitLabel }: Props) {
    const fields: CredentialField[] = provider.fields.length ? provider.fields : ['api_key']
    const [values, setValues] = useState<Partial<Record<CredentialField, string>>>(() =>
        Object.fromEntries(fields.filter((f) => f === 'endpoint' || f === 'base_url').map((f) => [f, provider.endpoint ?? ''])) as Partial<Record<CredentialField, string>>
    )
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()
    const [result, setResult] = useState<TestResult>()

    const ready = fields.some((f) => (values[f] ?? '').trim() !== '') || editing

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                setResult(undefined)
                try {
                    const body = Object.fromEntries(fields.filter((f) => (values[f] ?? '').trim() !== '').map((f) => [f, (values[f] ?? '').trim()]))
                    await saveProvider(provider.id, body)
                    const test = await testProvider(provider.id).catch(() => undefined)
                    if (test && !test.ok) {
                        setResult(test)
                        return
                    }
                    onDone()
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'It could not be saved.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            {fields.map((f, i) => (
                <TextField
                    key={f}
                    label={FIELD_LABELS[f]}
                    mono
                    type={PLAIN_FIELDS.includes(f) ? 'text' : 'password'}
                    autoComplete="off"
                    autoFocus={i === 0}
                    value={values[f] ?? ''}
                    placeholder={editing && !PLAIN_FIELDS.includes(f) ? 'Leave empty to keep the saved one' : undefined}
                    onChange={(e) => setValues((v) => ({ ...v, [f]: e.target.value }))}
                />
            ))}
            {provider.doc && (
                <a className="type-small text-muted underline" href={provider.doc} target="_blank" rel="noreferrer">
                    {provider.name} documentation
                </a>
            )}
            {error && <Notice tone="failed" title="It could not be saved">{error}</Notice>}
            {result && !result.ok && (
                <Notice
                    tone="retrying"
                    title="Saved, but the connection check failed"
                    actions={<Button onClick={onDone}>Keep it anyway</Button>}
                >
                    {result.error ?? `The provider answered ${result.status ?? 'with an error'}.`}
                </Notice>
            )}
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!ready}>
                    {submitLabel ?? (editing ? 'Save' : 'Connect')}
                </Button>
            </div>
        </form>
    )
}
