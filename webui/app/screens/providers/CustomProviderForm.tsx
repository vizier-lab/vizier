import { Button, FilterTabs, Notice, TextField, Textarea } from '@vizier/design'
import { useState } from 'react'
import { createCustom, testCustom, updateCustom, type CustomBody, type CustomModel, type CustomProtocol, type CustomProvider, type TestResult } from '../../api/endpoints/providers'

/**
 * The path Vizier appends to the base URL, as the transport builds it (`llm/dialect/openai_chat`
 * and `anthropic`): OpenAI chat is `{base}/chat/completions`; Anthropic is `{base}/messages`, with
 * `/v1` inserted in front when the base carries no version prefix.
 */
export function endpointFor(protocol: CustomProtocol, baseUrl: string): string {
    const base = baseUrl.trim().replace(/\/+$/, '')
    if (protocol === 'anthropic') {
        return base.endsWith('/v1') ? '/messages' : '/v1/messages'
    }
    return '/chat/completions'
}

/** The endpoints a pasted URL may already carry, longest first, so the longest is removed. */
const ENDPOINTS = ['/chat/completions', '/responses', '/completions', '/messages']

/**
 * The base URL without a trailing slash or endpoint. A value saved from a curl line
 * (`https://host/v1/chat/completions`) is shown — and saved again — as the base Vizier appends to.
 */
export function withoutEndpoint(baseUrl: string): string {
    const trimmed = baseUrl.trim().replace(/\/+$/, '')
    const tail = ENDPOINTS.find((endpoint) => trimmed.endsWith(endpoint))
    return tail ? trimmed.slice(0, -tail.length) : trimmed
}

type Props = {
    /** Editing this provider; absent creates one. */
    provider?: CustomProvider
    /** A name to start from, when the search found nothing to connect. */
    initialName?: string
    onDone: () => void
    onCancel: () => void
}

/** A provider of the operator's own: a protocol, a base URL, an optional key, and its model list. */
export function CustomProviderForm({ provider, initialName, onDone, onCancel }: Props) {
    const [name, setName] = useState(provider?.name ?? initialName ?? '')
    const [protocol, setProtocol] = useState<CustomProtocol>(provider?.protocol ?? 'openai_compatible')
    const [baseUrl, setBaseUrl] = useState(withoutEndpoint(provider?.base_url ?? ''))
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
            <TextField
                label="Base URL"
                mono
                value={baseUrl}
                placeholder="http://10.0.0.8:8000/v1"
                addon={endpointFor(protocol, baseUrl)}
                helper={`Up to the version prefix — Vizier adds ${endpointFor(protocol, baseUrl)} itself.`}
                onChange={(e) => setBaseUrl(e.target.value)}
            />
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
