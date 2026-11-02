import { Button, HighlightMatch, Icon, Notice, Picker, TextField, type PickerSection } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { api } from '../../api/client'
import type { ModelSummary } from '../../api/endpoints/catalogue'
import { allModels } from '../../api/endpoints/providers'

/** The connection that carries inference-profile entries (spec 044, FR-022). */
export const SSO_PROVIDER = 'amazon-bedrock-sso'

// The entry's wire shape and its calls. Temporary home: T047 moves the types and calls to
// `api/endpoints/providers.ts` (and the pricing rendering helpers to `lib/providers.ts`); this
// batch leaves that module alone, so the form and the drawer share one local definition.
export type EntryPrice = { input: number; output: number; cache_read?: number | null; cache_write?: number | null }

/** What the entry's usage costs right now (contracts §4a). Every absence carries its own reason. */
export type EntryPricing =
    | { state: 'explicit'; cost: EntryPrice; source?: string }
    | { state: 'from_catalogue'; cost: EntryPrice; model: string }
    | { state: 'not_resolved' | 'not_set' | 'no_catalogue'; flag: string; reason: string; model?: string }

/** One saved inference-profile entry as the API returns it (contracts §4). */
export type ProfileEntry = {
    arn: string
    name?: string | null
    context?: number | null
    output?: number | null
    tool_call?: boolean | null
    image_input?: boolean | null
    model?: string | null
    price?: EntryPrice | null
    edited?: boolean
    display_name: string
    pricing: EntryPricing
}

/** The whole entry, as the entry endpoints take it. */
export type ProfileEntryBody = {
    arn: string
    name: string | null
    context: number | null
    output: number | null
    tool_call: boolean | null
    image_input: boolean | null
    model: string | null
    price: EntryPrice | null
    edited: boolean
}

export const profileEntries = (id: string) => api.get<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles`)

export const addProfileEntry = (id: string, body: ProfileEntryBody) =>
    api.post<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles`, body)

/** Replaces the entry saved under `arn`; the ARN rides in the path so a person can verify it. */
export const saveProfileEntry = (id: string, arn: string, body: ProfileEntryBody, confirm = false) =>
    api.put<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles/${encodeURIComponent(arn)}${confirm ? '?confirm=true' : ''}`, body)

export const removeProfileEntry = (id: string, arn: string, confirm = false) =>
    api.delete<ProfileEntry[]>(`/providers/${encodeURIComponent(id)}/profiles/${encodeURIComponent(arn)}${confirm ? '?confirm=true' : ''}`)

/** A limit as the API takes it: `null` when empty, `NaN` when it is not a positive whole number. */
function tokens(value: string): number | null {
    const text = value.trim()
    if (!text) return null
    const n = Number(text)
    return Number.isSafeInteger(n) && n > 0 ? n : Number.NaN
}

/** A price rate: `null` when empty, `NaN` when it is not a finite, non-negative number. */
function rate(value: string): number | null {
    const text = value.trim()
    if (!text) return null
    const n = Number(text)
    return Number.isFinite(n) && n >= 0 ? n : Number.NaN
}

const trigger =
    'flex h-40 w-full items-center gap-8 rounded-md border border-border-strong bg-ground pr-10 pl-12 text-left type-body text-fg hover:border-border-hover focus-visible:border-live focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40'

type Props = {
    providerId: string
    /** The saved entry being edited; absent adds one. */
    entry?: ProfileEntry
    onSaved: (entries: ProfileEntry[]) => void
    onCancel: () => void
}

/**
 * One inference-profile entry (FR-034a): the ARN, an optional name and limits, the catalogue
 * model its usage is priced from — through the existing searchable `Picker`, never a new
 * component (rule W1) — and a hand-entered price that wins over the catalogue.
 */
export function ProfileEntryForm({ providerId, entry, onSaved, onCancel }: Props) {
    const [arn, setArn] = useState(entry?.arn ?? '')
    const [name, setName] = useState(entry?.name ?? '')
    const [context, setContext] = useState(entry?.context?.toString() ?? '')
    const [output, setOutput] = useState(entry?.output?.toString() ?? '')
    const [model, setModel] = useState<string | null>(entry?.model ?? null)
    const [modelQuery, setModelQuery] = useState('')
    const [priceInput, setPriceInput] = useState(entry?.price?.input?.toString() ?? '')
    const [priceOutput, setPriceOutput] = useState(entry?.price?.output?.toString() ?? '')
    const [models, setModels] = useState<ModelSummary[]>()
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string>()

    // The pricing dropdown is the catalogue's Bedrock list as loaded — no new endpoint and no AWS
    // call (contracts §5a, FR-034c). The ARN entries the same list carries are not models to price
    // from, so they are left out here.
    useEffect(() => {
        let live = true
        allModels(providerId).then(
            (all) => live && setModels((all ?? []).filter((m) => m.origin !== 'inference_profile')),
            () => live && setModels([])
        )
        return () => {
            live = false
        }
    }, [providerId])

    const chosen = models?.find((m) => m.id === model)
    const sections = useMemo<PickerSection<ModelSummary>[]>(() => {
        const q = modelQuery.trim().toLowerCase()
        return [{ items: (models ?? []).filter((m) => !q || m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q)) }]
    }, [models, modelQuery])

    const windowTokens = tokens(context)
    const outputTokens = tokens(output)
    const inputRate = rate(priceInput)
    const outputRate = rate(priceOutput)
    const priceTouched = priceInput.trim() !== '' || priceOutput.trim() !== ''
    const priceInvalid = priceTouched && (inputRate === null || outputRate === null || Number.isNaN(inputRate) || Number.isNaN(outputRate))
    const price: EntryPrice | null =
        priceTouched && !priceInvalid && inputRate !== null && outputRate !== null
            ? { input: inputRate, output: outputRate, cache_read: entry?.price?.cache_read ?? null, cache_write: entry?.price?.cache_write ?? null }
            : null
    const invalidLimit = Number.isNaN(windowTokens) || Number.isNaN(outputTokens)
    const ready = arn.trim() !== '' && !invalidLimit && !priceInvalid

    const save = async () => {
        if (!ready) return
        const body: ProfileEntryBody = {
            arn: arn.trim(),
            name: name.trim() || null,
            context: windowTokens,
            output: outputTokens,
            tool_call: entry?.tool_call ?? null,
            image_input: entry?.image_input ?? null,
            model,
            price,
            edited: true,
        }
        setBusy(true)
        setError(undefined)
        try {
            const saved = entry ? await saveProfileEntry(providerId, entry.arn, body) : await addProfileEntry(providerId, body)
            onSaved(saved)
        } catch (e) {
            setError(e instanceof Error ? e.message : 'The entry could not be saved.')
        } finally {
            setBusy(false)
        }
    }

    return (
        <form
            className="flex flex-col gap-16 rounded-lg border border-border bg-surface p-16"
            onSubmit={(e) => {
                e.preventDefault()
                void save()
            }}
        >
            <span className="type-ui-medium text-fg">{entry ? 'Edit inference profile' : 'Add an inference profile'}</span>
            <TextField
                label="ARN"
                mono
                autoFocus={!entry}
                locked={entry !== undefined}
                value={arn}
                placeholder="arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123"
                helper={
                    entry
                        ? 'The ARN identifies the entry. Add a new one to point at a different profile.'
                        : 'An application or system inference profile ARN, exactly as AWS shows it. Vizier sends it unchanged.'
                }
                onChange={(e) => setArn(e.target.value)}
            />
            <TextField
                label="Name (optional)"
                value={name}
                placeholder="Acme Prod — Claude"
                helper="Shown instead of the profile id, so people can tell the entries apart."
                onChange={(e) => setName(e.target.value)}
            />
            <div className="grid grid-cols-1 gap-12 sm:grid-cols-2">
                <TextField
                    label="Context window (optional)"
                    type="number"
                    inputMode="numeric"
                    suffix="tokens"
                    value={context}
                    invalid={Number.isNaN(windowTokens)}
                    helper={Number.isNaN(windowTokens) ? 'Use a whole number of tokens.' : 'Leave empty when unknown.'}
                    onChange={(e) => setContext(e.target.value)}
                />
                <TextField
                    label="Max output (optional)"
                    type="number"
                    inputMode="numeric"
                    suffix="tokens"
                    value={output}
                    invalid={Number.isNaN(outputTokens)}
                    helper={Number.isNaN(outputTokens) ? 'Use a whole number of tokens.' : 'Leave empty when unknown.'}
                    onChange={(e) => setOutput(e.target.value)}
                />
            </div>

            <div className="flex flex-col gap-6">
                <span className="type-ui-medium text-soft">Model the usage is priced from</span>
                <Picker<ModelSummary>
                    label="Catalogue models to price from"
                    width={520}
                    itemHeight={52}
                    trigger={
                        <button type="button" aria-label={`Pricing model: ${model ?? 'none chosen'}`} className={trigger}>
                            {model ? (
                                <span className="flex min-w-0 flex-1 items-baseline gap-8">
                                    <span className="truncate">{chosen?.name ?? model}</span>
                                    {chosen && chosen.name !== model && <span className="truncate type-mono-xs text-faint">{model}</span>}
                                    {!chosen && models && <span className="shrink-0 type-small text-retrying">not in the catalogue</span>}
                                </span>
                            ) : (
                                <span className="min-w-0 flex-1 truncate text-faint">No model — price by hand or leave unpriced</span>
                            )}
                            <Icon name="chevron-down" size={14} tone="muted" />
                        </button>
                    }
                    search={{ placeholder: 'Search models by name or id', total: models ? `${models.length}` : undefined, value: modelQuery, onChange: setModelQuery }}
                    sections={sections}
                    getKey={(m) => m.id}
                    loading={!models}
                    empty={models ? 'No Bedrock model matched.' : undefined}
                    renderItem={(m, { query }) => (
                        <div className="flex min-w-0 flex-1 flex-col">
                            <span className="truncate type-ui text-fg">
                                <HighlightMatch text={m.name} query={query} />
                            </span>
                            <span className="truncate type-mono-xs text-faint">{m.id}</span>
                        </div>
                    )}
                    onSelect={(m) => {
                        setModel(m.id)
                        setModelQuery('')
                    }}
                    footer={
                        model ? (
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={() => {
                                    setModel(null)
                                    setModelQuery('')
                                }}
                            >
                                Clear the model
                            </Button>
                        ) : undefined
                    }
                />
                <span className="type-small text-faint">Its catalogue price costs this entry’s usage. It never supplies the limits above.</span>
            </div>

            <div className="flex flex-col gap-6">
                <span className="type-ui-medium text-soft">Or set a price by hand (USD per 1M tokens)</span>
                <div className="grid grid-cols-1 gap-12 sm:grid-cols-2">
                    <TextField
                        label="Input"
                        type="number"
                        inputMode="decimal"
                        value={priceInput}
                        invalid={priceInvalid}
                        onChange={(e) => setPriceInput(e.target.value)}
                    />
                    <TextField
                        label="Output"
                        type="number"
                        inputMode="decimal"
                        value={priceOutput}
                        invalid={priceInvalid}
                        onChange={(e) => setPriceOutput(e.target.value)}
                    />
                </div>
                <span className="type-small text-faint">
                    {priceInvalid ? 'Enter both rates as non-negative numbers, or leave both empty.' : 'Wins over the catalogue model. Leave both empty to use it.'}
                </span>
            </div>

            {error && (
                <Notice tone="failed" title="The entry could not be saved">
                    {error}
                </Notice>
            )}
            <div className="flex justify-end gap-8">
                <Button type="button" variant="ghost" onClick={onCancel}>
                    Cancel
                </Button>
                <Button type="submit" variant="primary" loading={busy} disabled={!ready}>
                    {entry ? 'Save entry' : 'Add entry'}
                </Button>
            </div>
        </form>
    )
}
