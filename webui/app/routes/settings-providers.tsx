import { Button, Dialog, EmptyState, Menu, MenuItem, Presence, ProviderRow, Skeleton, popoverMotion, useDismiss } from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { motion } from 'motion/react'
import { useNavigate } from 'react-router'
import { ApiError } from '../api/client'
import { allProviders, type ProviderSummary } from '../api/endpoints/catalogue'
import {
    catalogueStatus,
    connectedProviders,
    customProviders,
    deleteCustom,
    deleteProvider,
    refreshCatalogue,
    type AffectedAgent,
    type CatalogueStatus,
    type ConnectedProvider,
    type CustomProvider,
} from '../api/endpoints/providers'
import { relativeTime } from '../lib/format'
import { credentialText, customCredentialText, inUseMessage, usageText } from '../lib/providers'
import { invalidateCatalogue, logoOf } from '../lib/catalogue'
import { ConnectForm } from '../screens/providers/ConnectForm'
import { CustomProviderForm } from '../screens/providers/CustomProviderForm'
import { ModelsDrawer } from '../screens/providers/ModelsDrawer'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

type Row =
    | { kind: 'catalogue'; provider: ConnectedProvider }
    | { kind: 'custom'; provider: CustomProvider }

type Pending = { row: Row; agents: AffectedAgent[] }

export default function SettingsProviders() {
    const navigate = useNavigate()
    const [connected, setConnected] = useState<ConnectedProvider[]>()
    const [custom, setCustom] = useState<CustomProvider[]>([])
    const [catalogue, setCatalogue] = useState<ProviderSummary[]>([])
    const [status, setStatus] = useState<CatalogueStatus>()
    const [error, setError] = useState<string>()
    const [menu, setMenu] = useState<string | null>(null)
    const [connecting, setConnecting] = useState<{ provider: ProviderSummary; editing: boolean } | null>(null)
    const [editingCustom, setEditingCustom] = useState<CustomProvider | null>(null)
    const [models, setModels] = useState<{ id: string; name: string; custom: boolean } | null>(null)
    const [pending, setPending] = useState<Pending | null>(null)
    const [refreshing, setRefreshing] = useState(false)

    const load = useCallback(async () => {
        invalidateCatalogue()
        try {
            const [c, cu, all, st] = await Promise.all([connectedProviders(), customProviders(), allProviders(), catalogueStatus().catch(() => undefined)])
            setConnected(c ?? [])
            setCustom(cu ?? [])
            setCatalogue(all ?? [])
            setStatus(st)
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Providers could not be loaded.')
        }
    }, [])
    useEffect(() => void load(), [load])

    // Close the ⋯ menu on Escape or a click elsewhere.
    const menuRoot = useRef<HTMLDivElement>(null)
    useDismiss(menu !== null, () => setMenu(null), menuRoot)

    const rows: Row[] = useMemo(
        () => [...(connected ?? []).map((provider) => ({ kind: 'catalogue' as const, provider })), ...custom.map((provider) => ({ kind: 'custom' as const, provider }))],
        [connected, custom]
    )

    const remove = async (row: Row, confirm: boolean) => {
        try {
            if (row.kind === 'custom') await deleteCustom(row.provider.id, confirm)
            else await deleteProvider(row.provider.id, confirm)
            setPending(null)
            toast.success(`${row.provider.name} removed`)
            await load()
        } catch (e) {
            if (e instanceof ApiError && e.status === 409) {
                const agents = (e.data as { agents?: AffectedAgent[] } | undefined)?.agents ?? []
                setPending({ row, agents })
                return
            }
            setPending(null)
            toast.error('Couldn’t remove the provider', e instanceof Error ? e.message : undefined)
        }
    }

    const refresh = async () => {
        setRefreshing(true)
        try {
            setStatus(await refreshCatalogue())
            setCatalogue((await allProviders()) ?? [])
            toast.success('Catalogue refreshed')
        } catch (e) {
            toast.error('Couldn’t refresh the catalogue', e instanceof Error ? e.message : undefined)
        } finally {
            setRefreshing(false)
        }
    }

    const summaryOf = (id: string) => catalogue.find((p) => p.id === id)
    const rowMenu = (row: Row) => (
        <motion.div
            key={row.provider.id}
            className="absolute top-44 right-16 z-30"
            ref={menuRoot}
            {...popoverMotion}
        >
            <Menu aria-label={`${row.provider.name} actions`}>
                <MenuItem
                    icon="pencil"
                    label={row.kind === 'custom' ? 'Edit' : 'Replace credentials'}
                    onClick={() => {
                        setMenu(null)
                        if (row.kind === 'custom') setEditingCustom(row.provider)
                        else {
                            const s = summaryOf(row.provider.id)
                            if (s) setConnecting({ provider: s, editing: true })
                        }
                    }}
                />
                <MenuItem
                    icon="trash"
                    tone="danger"
                    label={row.kind === 'catalogue' && row.provider.source?.startsWith('env:') && !row.provider.has_api_key ? 'Delete saved credentials' : 'Delete'}
                    onClick={() => {
                        setMenu(null)
                        void remove(row, false)
                    }}
                />
            </Menu>
        </motion.div>
    )

    return (
        <PageFrame
            title="Providers"
            meta={`${rows.length} connected · catalogue from models.dev${status ? `, refreshed ${relativeTime(status.checked_at ?? status.fetched_at)}` : ''}`}
            description="Keys are stored in Vizier; environment variables still work."
            actions={
                <>
                    <Button icon="refresh" loading={refreshing} onClick={refresh}>
                        Refresh catalogue
                    </Button>
                    <Button variant="primary" icon="plus" onClick={() => navigate('/settings/providers/add')}>
                        Add provider
                    </Button>
                </>
            }
        >
            {error ? (
                <EmptyState kind="error" title="Couldn’t load providers" message={error} />
            ) : !connected ? (
                <Skeleton className="h-120" />
            ) : rows.length === 0 ? (
                <EmptyState
                    kind="empty"
                    icon="cloud"
                    title="Nothing is connected yet"
                    message="Connect a provider to give your agents a model. Ollama runs models on your own machine, no key needed."
                    action={
                        <Button variant="primary" icon="plus" onClick={() => navigate('/settings/providers/add')}>
                            Add provider
                        </Button>
                    }
                />
            ) : (
                <section aria-label="Connected providers" className="-mx-16 flex flex-col">
                    {rows.map((row) => (
                        <div key={`${row.kind}:${row.provider.id}`} className="relative">
                            <ProviderRow
                                provider={row.provider.id}
                                name={row.provider.name}
                                logo={logoOf(summaryOf(row.provider.id))}
                                custom={row.kind === 'custom'}
                                credential={row.kind === 'custom' ? customCredentialText(row.provider) : credentialText(row.provider)}
                                usage={usageText(row.provider.used_by, row.kind === 'custom' ? row.provider.models.length : row.provider.model_count)}
                                onModels={() => setModels({ id: row.provider.id, name: row.provider.name, custom: row.kind === 'custom' })}
                                onMore={() => setMenu(menu === row.provider.id ? null : row.provider.id)}
                            />
                            <Presence>
                                {menu === row.provider.id && rowMenu(row)}
                            </Presence>
                        </div>
                    ))}
                </section>
            )}

            <Dialog open={connecting !== null} title={connecting ? `${connecting.editing ? 'Replace credentials for' : 'Connect'} ${connecting.provider.name}` : ''} confirmLabel="" onConfirm={() => {}} onCancel={() => setConnecting(null)}>
                {connecting && (
                    <ConnectForm
                        key={connecting.provider.id}
                        provider={connecting.provider}
                        editing={connecting.editing}
                        onCancel={() => setConnecting(null)}
                        onDone={() => {
                            setConnecting(null)
                            toast.success(`${connecting.provider.name} connected`)
                            void load()
                        }}
                    />
                )}
            </Dialog>

            <Dialog open={editingCustom !== null} title="Edit custom provider" confirmLabel="" onConfirm={() => {}} onCancel={() => setEditingCustom(null)}>
                {editingCustom !== null && (
                    <CustomProviderForm
                        key={editingCustom.id}
                        provider={editingCustom}
                        onCancel={() => setEditingCustom(null)}
                        onDone={() => {
                            setEditingCustom(null)
                            void load()
                        }}
                    />
                )}
            </Dialog>

            <ModelsDrawer provider={models} onClose={() => setModels(null)} onFetched={() => void load()} />

            <ConfirmDialog
                open={pending !== null}
                title={`Remove ${pending?.row.provider.name ?? 'provider'}?`}
                message={pending ? `These agents use it and will stop working until they get another model: ${inUseMessage(pending.agents)}.` : undefined}
                confirmLabel="Remove anyway"
                onConfirm={() => pending && void remove(pending.row, true)}
                onCancel={() => setPending(null)}
            />
        </PageFrame>
    )
}
