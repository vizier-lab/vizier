import { Button, IconButton, Notice, SearchField, SlideOver, Skeleton } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { ApiError } from '../../api/client'
import type { ModelSummary } from '../../api/endpoints/catalogue'
import {
    allModels,
    fetchCustomModels,
    profileEntries,
    removeProfileEntry,
    type AffectedAgent,
    type EntryPricing,
    type ProfileEntry,
} from '../../api/endpoints/providers'
import { capabilityLine } from '../../lib/models'
import { entryLimits, pricingCandidates, pricingNote, SSO_PROVIDER } from '../../lib/providers'
import { toast } from '../../stores/toasts'
import { ConfirmDialog } from '../shared/ConfirmDialog'
import { ProfileEntryForm } from './ProfileEntryForm'

type Props = {
    /** The provider whose models are shown; null closes the drawer. */
    provider: { id: string; name: string; custom: boolean } | null
    onClose: () => void
    /** After models were fetched from a custom provider's server, or an entry was saved. */
    onFetched: () => void
}

/**
 * What the entry's usage costs — or, when it costs nothing yet, the label **and** the
 * cause-specific reason the server derives on every read (contracts §4a, FR-034a.2, FR-034a.3,
 * FR-034b, SC-021). The note's shape and wording are `lib/providers.ts`; this renders them as
 * they come, so the three absences stay distinct and visible without a click.
 */
export function PricingNote({ pricing }: { pricing: EntryPricing }) {
    const note = pricingNote(pricing)
    if (note.priced) return <span className="type-small text-muted">{note.text}</span>
    return (
        <div className="flex flex-col gap-2">
            <span className="type-ui-medium text-retrying">{note.flag}</span>
            <span className="type-small text-muted">{note.reason}</span>
        </div>
    )
}

type EntryRowProps = {
    entry: ProfileEntry
    onEdit?: () => void
    onRemove?: () => void
}

/**
 * One saved inference-profile entry: the name, the **ARN in full** so a person can verify which
 * profile it is (FR-022, FR-026), its limits, and its pricing state with the reason beside it.
 */
export function EntryRow({ entry, onEdit, onRemove }: EntryRowProps) {
    const limits = entryLimits(entry)
    return (
        <li className="flex flex-col gap-6 border-b border-border py-12 last:border-b-0">
            <div className="flex items-start justify-between gap-8">
                <div className="flex min-w-0 flex-col gap-2">
                    <span className="type-ui text-fg">{entry.display_name}</span>
                    <span className="type-code break-all text-faint">{entry.arn}</span>
                </div>
                {(onEdit || onRemove) && (
                    <div className="flex shrink-0 gap-4">
                        {onEdit && <IconButton icon="pencil" size="sm" aria-label={`Edit ${entry.display_name}`} onClick={onEdit} />}
                        {onRemove && <IconButton icon="trash" size="sm" aria-label={`Remove ${entry.display_name}`} onClick={onRemove} />}
                    </div>
                )}
            </div>
            <span className="type-small text-faint">{limits}</span>
            <PricingNote pricing={entry.pricing} />
        </li>
    )
}

/** Every model a provider offers, with what it can do and costs — and, on the SSO connection, its
 *  inference-profile entries with their pricing state and add/edit/remove (FR-022, FR-026). */
export function ModelsDrawer({ provider, onClose, onFetched }: Props) {
    const [models, setModels] = useState<ModelSummary[]>()
    const [query, setQuery] = useState('')
    const [fetching, setFetching] = useState(false)
    const [entries, setEntries] = useState<ProfileEntry[]>()
    const [entriesError, setEntriesError] = useState<string>()
    const [editing, setEditing] = useState<ProfileEntry | 'new' | null>(null)
    const [removing, setRemoving] = useState<{ entry: ProfileEntry; agents: AffectedAgent[] } | null>(null)
    const [removeBusy, setRemoveBusy] = useState(false)
    const sso = provider?.id === SSO_PROVIDER

    useEffect(() => {
        setModels(undefined)
        setQuery('')
        setEntries(undefined)
        setEntriesError(undefined)
        setEditing(null)
        setRemoving(null)
        if (!provider) return
        let live = true
        allModels(provider.id).then(
            (m) => live && setModels(m ?? []),
            () => live && setModels([])
        )
        if (provider.id === SSO_PROVIDER) {
            profileEntries(provider.id).then(
                (e) => live && setEntries(e ?? []),
                (e) => live && setEntriesError(e instanceof Error ? e.message : 'The inference profiles could not be loaded.')
            )
        }
        return () => {
            live = false
        }
    }, [provider])

    // The SSO list also carries the saved entries themselves; they are the section above, not
    // models to list twice.
    const catalogueModels = useMemo(() => pricingCandidates(models ?? []), [models])
    const shown = useMemo(() => {
        const q = query.trim().toLowerCase()
        return catalogueModels.filter((m) => !q || m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q))
    }, [catalogueModels, query])

    const remove = async (entry: ProfileEntry, confirm: boolean) => {
        if (!provider) return
        setRemoveBusy(true)
        try {
            setEntries(await removeProfileEntry(provider.id, entry.arn, confirm))
            setRemoving(null)
            toast.success(`${entry.display_name} removed`)
            onFetched()
        } catch (e) {
            if (e instanceof ApiError && e.status === 409) {
                const agents = (e.data as { agents?: AffectedAgent[] } | undefined)?.agents ?? []
                setRemoving({ entry, agents })
                return
            }
            setRemoving(null)
            toast.error('Couldn’t remove the inference profile', e instanceof Error ? e.message : undefined)
        } finally {
            setRemoveBusy(false)
        }
    }

    return (
        <>
            <SlideOver
                open={provider !== null}
                title={provider ? `${provider.name} models` : ''}
                onClose={onClose}
                footer={
                    provider?.custom ? (
                        <Button
                            loading={fetching}
                            onClick={async () => {
                                setFetching(true)
                                try {
                                    await fetchCustomModels(provider.id)
                                    setModels(await allModels(provider.id))
                                    onFetched()
                                } catch (e) {
                                    toast.error('Couldn’t fetch the model list', e instanceof Error ? e.message : undefined)
                                } finally {
                                    setFetching(false)
                                }
                            }}
                        >
                            Fetch models from the server
                        </Button>
                    ) : undefined
                }
            >
                <div className="flex flex-col gap-12">
                    {sso && (
                        <section aria-label="Inference profiles" className="flex flex-col gap-8">
                            <div className="flex items-start justify-between gap-8">
                                <div className="flex flex-col gap-2">
                                    <span className="type-ui-medium text-fg">Inference profiles</span>
                                    <span className="type-small text-faint">Each saved ARN is one selectable model on this connection.</span>
                                </div>
                                <Button size="sm" icon="plus" onClick={() => setEditing('new')}>
                                    Add entry
                                </Button>
                            </div>
                            {entriesError ? (
                                <Notice tone="failed" title="Couldn’t load the inference profiles">
                                    {entriesError}
                                </Notice>
                            ) : !entries ? (
                                <Skeleton className="h-80" />
                            ) : entries.length === 0 ? (
                                <p className="type-small text-faint">No inference profiles yet. Add the ARN of an application or system inference profile.</p>
                            ) : (
                                <ul className="flex flex-col">
                                    {entries.map((entry) => (
                                        <EntryRow
                                            key={entry.arn}
                                            entry={entry}
                                            onEdit={() => setEditing(entry)}
                                            onRemove={() => void remove(entry, false)}
                                        />
                                    ))}
                                </ul>
                            )}
                            {editing && (
                                <ProfileEntryForm
                                    key={editing === 'new' ? 'new' : editing.arn}
                                    providerId={SSO_PROVIDER}
                                    entry={editing === 'new' ? undefined : editing}
                                    onSaved={(list) => {
                                        setEntries(list)
                                        setEditing(null)
                                        toast.success('Inference profile saved')
                                        onFetched()
                                    }}
                                    onCancel={() => setEditing(null)}
                                />
                            )}
                        </section>
                    )}

                    <SearchField aria-label="Search models" placeholder="Search models" value={query} onValueChange={setQuery} count={models ? `${shown.length}` : undefined} />
                    {!models ? (
                        <Skeleton className="h-120" />
                    ) : shown.length === 0 ? (
                        <p className="type-small text-faint">{catalogueModels.length === 0 ? 'No models are listed.' : 'Nothing matched.'}</p>
                    ) : (
                        <ul className="flex flex-col divide-y divide-border">
                            {shown.map((m) => (
                                <li key={m.id} className="flex flex-col gap-2 py-10">
                                    <span className="flex items-center gap-8">
                                        <span className="type-code text-fg">{m.id}</span>
                                        {m.status !== 'active' && <span className="type-mono-xs text-retrying">{m.status}</span>}
                                    </span>
                                    <span className="type-small text-faint">{capabilityLine(m)}</span>
                                </li>
                            ))}
                        </ul>
                    )}
                </div>
            </SlideOver>

            <ConfirmDialog
                open={removing !== null}
                title={`Remove ${removing?.entry.display_name ?? 'this inference profile'}?`}
                message={
                    removing && removing.agents.length > 0
                        ? 'These agents still chat through it and will stop working until they get another model:'
                        : 'Its ARN stops being a model on this connection.'
                }
                removes={removing?.agents.map((a) => `${a.name} (${a.features.join(', ')})`)}
                confirmLabel="Remove anyway"
                busy={removeBusy}
                onConfirm={() => removing && void remove(removing.entry, true)}
                onCancel={() => setRemoving(null)}
            />
        </>
    )
}
