import { Button, FilterTabs, Notice, TextField, Textarea } from '@vizier/design'
import { useState } from 'react'
import { createCustom, testCustom, updateCustom, type CustomBody, type CustomModel, type CustomProtocol, type CustomProvider, type TestResult } from '../../api/endpoints/providers'

type Props = {
    /** Editing this provider; absent creates one. */
    provider?: CustomProvider
    onDone: () => void
    onCancel: () => void
}

/** A provider of the operator's own: a protocol, a base URL, an optional key, and its model list. */
export function CustomProviderForm({ provider, onDone, onCancel }: Props) {
    const [name, setName] = useState(provider?.name ?? '')
    const [protocol, setProtocol] = useState<CustomProtocol>(provider?.protocol ?? 'openai_compatible')
    const [baseUrl, setBaseUrl] = useState(provider?.base_url ?? '')
    const [apiKey, setApiKey] = useState('')
    const [models, setModels] = useState((provider?.models ?? []).map((m) => m.id).join('\n'))
    const [busy, setBusy] = useState(false)
    const [checking, setChecking] = useState(false)
    const [error, setError] = useState<string>()
    const [test, setTest] = useState<TestResult>()

    const body = (): CustomBody => {
        const ids = models.split('\n').map((m) => m.trim()).filter(Boolean)
        const rows: CustomModel[] = ids.map((id) => provider?.models.find((m) => m.id === id) ?? { id })
        return { name: name.trim(), protocol, base_url: baseUrl.trim(), ...(apiKey ? { api_key: apiKey } : {}), models: rows }
    }
    const ready = name.trim() !== '' && baseUrl.trim() !== ''

    return (
        <form
            className="flex flex-col gap-16"
            onSubmit={async (e) => {
                e.preventDefault()
                setBusy(true)
                setError(undefined)
                try {
                    if (provider) await updateCustom(provider.id, body())
                    else await createCustom(body())
                    onDone()
                } catch (err) {
                    setError(err instanceof Error ? err.message : 'It could not be saved.')
                } finally {
                    setBusy(false)
                }
            }}
        >
            <TextField label="Name" value={name} autoFocus onChange={(e) => setName(e.target.value)} />
            <FilterTabs
                aria-label="Protocol"
                options={[
                    { value: 'openai_compatible', label: 'OpenAI-compatible' },
                    { value: 'anthropic', label: 'Anthropic protocol' },
                ]}
                value={protocol}
                onChange={setProtocol}
            />
            <TextField label="Base URL" mono value={baseUrl} placeholder="http://10.0.0.8:8000/v1" onChange={(e) => setBaseUrl(e.target.value)} />
            <TextField
                label="API key"
                mono
                type="password"
                autoComplete="off"
                value={apiKey}
                helper={provider?.has_api_key ? 'Leave empty to keep the saved key.' : 'Optional.'}
                onChange={(e) => setApiKey(e.target.value)}
            />
            <Textarea label="Models" mono rows={4} value={models} helper="One model id per line. Context sizes and prices fill in from the catalogue where it knows the model." onChange={(e) => setModels(e.target.value)} />
            {error && <Notice tone="failed" title="It could not be saved">{error}</Notice>}
            {test && (
                <Notice tone={test.ok ? 'info' : 'retrying'} title={test.ok ? `Reachable${test.model_count != null ? ` · ${test.model_count} models listed` : ''}` : 'Not reachable'}>
                    {test.ok ? undefined : (test.error ?? `It answered ${test.status ?? 'with an error'}.`)}
                </Notice>
            )}
            <div className="flex items-center justify-between gap-8">
                <Button
                    type="button"
                    loading={checking}
                    disabled={!ready}
                    onClick={async () => {
                        setChecking(true)
                        setTest(undefined)
                        try {
                            setTest(await testCustom(body()))
                        } catch (err) {
                            setTest({ ok: false, error: err instanceof Error ? err.message : undefined })
                        } finally {
                            setChecking(false)
                        }
                    }}
                >
                    Test connection
                </Button>
                <div className="flex gap-8">
                    <Button type="button" variant="ghost" onClick={onCancel}>
                        Cancel
                    </Button>
                    <Button type="submit" variant="primary" loading={busy} disabled={!ready}>
                        {provider ? 'Save provider' : 'Add provider'}
                    </Button>
                </div>
            </div>
        </form>
    )
}
