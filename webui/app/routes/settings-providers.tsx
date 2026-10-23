import { Button, Card, Dialog, EmptyState, Eyebrow, Menu, MenuItem, PageHeader, ProviderRow, ProviderTile, SearchField, Skeleton } from '@vizier/design'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
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
import { ConnectForm } from '../screens/providers/ConnectForm'
import { CustomProviderForm } from '../screens/providers/CustomProviderForm'
import { ModelsDrawer } from '../screens/providers/ModelsDrawer'
import { ConfirmDialog } from '../screens/shared/ConfirmDialog'
import { toast } from '../stores/toasts'

type Row =
    | { kind: 'catalogue'; provider: ConnectedProvider }
    | { kind: 'custom'; provider: CustomProvider }

type Pending = { row: Row; agents: AffectedAgent[] }

const SHOWN_AT_MOST = 24

export default function SettingsProviders() {
    const [connected, setConnected] = useState<ConnectedProvider[]>()
    const [custom, setCustom] = useState<CustomProvider[]>([])
    const [catalogue, setCatalogue] = useState<ProviderSummary[]>([])
    const [status, setStatus] = useState<CatalogueStatus>()
    const [error, setError] = useState<string>()
    const [query, setQuery] = useState('')
    const [menu, setMenu] = useState<string | null>(null)
    const [connecting, setConnecting] = useState<{ provider: ProviderSummary; editing: boolean } | null>(null)
    const [editingCustom, setEditingCustom] = useState<CustomProvider | 'new' | null>(null)
    const [models, setModels] = useState<{ id: string; name: string; custom: boolean } | null>(null)
    const [pending, setPending] = useState<Pending | null>(null)
    const [refreshing, setRefreshing] = useState(false)

    const load = useCallback(async () => {
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

    // Close the ⋯ menu on a click elsewhere.
    const menuRoot = useRef<HTMLDivElement>(null)
    useEffect(() => {
        if (!menu) return
        const close = (e: PointerEvent) => !menuRoot.current?.contains(e.target as Node) && setMenu(null)
        document.addEventListener('pointerdown', close)
        return () => document.removeEventListener('pointerdown', close)
    }, [menu])

    const rows: Row[] = useMemo(
        () => [...(connected ?? []).map((provider) => ({ kind: 'catalogue' as const, provider })), ...custom.map((provider) => ({ kind: 'custom' as const, provider }))],
        [connected, custom]
    )

    const connectedIds = new Set(rows.map((r) => r.provider.id))
    const available = useMemo(() => {
        const q = query.trim().toLowerCase()
        const free = catalogue.filter((p) => !connectedIds.has(p.id) && p.origin !== 'custom' && !(p.fields.length === 0))
        const list = q ? free.filter((p) => p.name.toLowerCase().includes(q) || p.id.includes(q)) : free.filter((p) => p.popular)
        return { list: list.slice(0, SHOWN_AT_MOST), total: free.length, more: Math.max(0, list.length - SHOWN_AT_MOST) }
    }, [catalogue, query, connected, custom]) // eslint-disable-line react-hooks/exhaustive-deps

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
        <div className="absolute top-44 right-16 z-30" ref={menuRoot}>
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
        </div>
    )

    return (
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
            <div className="mx-auto flex w-full max-w-4xl flex-col gap-24 px-32 py-32">
                <PageHeader
                    size="settings"
                    title="Providers"
                    description={`Catalogue from models.dev${status ? ` · refreshed ${relativeTime(status.checked_at ?? status.fetched_at)}` : ''}. Keys are stored in Vizier; environment variables still work.`}
                    actions={
                        <Button icon="refresh" loading={refreshing} onClick={refresh}>
                            Refresh catalogue
                        </Button>
                    }
                />

                {error ? (
                    <EmptyState kind="error" title="Couldn’t load providers" message={error} />
                ) : !connected ? (
                    <Skeleton className="h-120" />
                ) : (
                    <>
                        <Card>
                            <Eyebrow count={rows.length}>Connected</Eyebrow>
                            {rows.length === 0 ? (
                                <p className="pt-8 type-small text-faint">Nothing is connected yet. Pick a provider below to give your agents a model.</p>
                            ) : (
                                <div className="flex flex-col">
                                    {rows.map((row) => (
                                        <div key={`${row.kind}:${row.provider.id}`} className="relative">
                                            <ProviderRow
                                                provider={row.provider.id}
                                                name={row.provider.name}
                                                custom={row.kind === 'custom'}
                                                credential={row.kind === 'custom' ? customCredentialText(row.provider) : credentialText(row.provider)}
                                                usage={usageText(row.provider.used_by, row.kind === 'custom' ? row.provider.models.length : row.provider.model_count)}
                                                onModels={() => setModels({ id: row.provider.id, name: row.provider.name, custom: row.kind === 'custom' })}
                                                onMore={() => setMenu(menu === row.provider.id ? null : row.provider.id)}
                                            />
                                            {menu === row.provider.id && rowMenu(row)}
                                        </div>
                                    ))}
                                </div>
                            )}
                        </Card>

                        <Card
                            title="Add a provider"
                            headerTrailing={<SearchField aria-label="Search providers" placeholder={`Search ${available.total} providers`} value={query} onValueChange={setQuery} className="w-260" />}
                        >
                            <div className="flex flex-col gap-16">
                                <Eyebrow>{query.trim() ? 'Results' : 'Popular'}</Eyebrow>
                                {available.list.length === 0 ? (
                                    <p className="type-small text-faint">{query.trim() ? 'No provider matched.' : 'Every popular provider is connected.'}</p>
                                ) : (
                                    <div className="grid grid-cols-1 gap-12 sm:grid-cols-2 lg:grid-cols-3">
                                        {available.list.map((p) => (
                                            <ProviderTile key={p.id} provider={p.id} name={p.name} onClick={() => setConnecting({ provider: p, editing: false })} />
                                        ))}
                                    </div>
                                )}
                                <div className="flex items-center gap-16">
                                    <Button icon="plus" onClick={() => setEditingCustom('new')}>
                                        Add custom provider
                                    </Button>
                                    <p className="type-small text-faint">Any OpenAI-compatible or Anthropic-protocol endpoint, with its own model list.</p>
                                </div>
                            </div>
                        </Card>
                    </>
                )}
            </div>

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

            <Dialog open={editingCustom !== null} title={editingCustom === 'new' ? 'Add custom provider' : 'Edit custom provider'} confirmLabel="" onConfirm={() => {}} onCancel={() => setEditingCustom(null)}>
                {editingCustom !== null && (
                    <CustomProviderForm
                        key={editingCustom === 'new' ? 'new' : editingCustom.id}
                        provider={editingCustom === 'new' ? undefined : editingCustom}
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
        </div>
    )
}
