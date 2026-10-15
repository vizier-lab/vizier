import { useEffect, useState } from 'react'
import {
  deleteCustomProvider,
  deleteProvider,
  fetchCustomProviderModels,
  getCatalogue,
  listCatalogueProviders,
  listCustomProviders,
  listProviders,
  refreshCatalogue,
} from '../../services/vizier'
import { useToastStore } from '../../hooks/toastStore'
import { useCatalogueStore } from '../../hooks/catalogueStore'
import type {
  AffectedAgent,
  CatalogueProvider,
  CatalogueStatus,
  CustomProviderBody,
  CustomProviderResponse,
  ProviderResponse,
} from '../../interfaces/types'
import SlideOver from '../SlideOver'
import ConnectedList, { type ConnectedRow } from './ConnectedList'
import AddProvider from './AddProvider'
import ConnectForm from './ConnectForm'
import CustomProviderForm from './CustomProviderForm'
import ModelsDrawer from './ModelsDrawer'
import DeleteWarning from './DeleteWarning'
import { errorMessage, hintStyle, secondaryButton } from './styles'

type Panel =
  | { kind: 'connect'; provider: CatalogueProvider; existing?: ProviderResponse }
  | { kind: 'custom'; existing?: CustomProviderResponse; initial?: Partial<CustomProviderBody> }

function rowId(row: ConnectedRow) {
  return row.kind === 'custom' ? row.custom.id : row.provider.id
}

function rowName(row: ConnectedRow) {
  return row.kind === 'custom' ? row.custom.name : row.provider.name
}

/** Settings › Providers: catalogue status, connected providers, and adding one. */
export default function ProvidersSection() {
  const addToast = useToastStore((s) => s.addToast)
  const invalidatePickers = useCatalogueStore((s) => s.invalidate)
  const [status, setStatus] = useState<CatalogueStatus | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [connected, setConnected] = useState<ProviderResponse[]>([])
  const [customs, setCustoms] = useState<CustomProviderResponse[]>([])
  const [catalogue, setCatalogue] = useState<CatalogueProvider[]>([])
  const [loading, setLoading] = useState(true)
  const [panel, setPanel] = useState<Panel | null>(null)
  const [drawer, setDrawer] = useState<{ id: string; name: string } | null>(null)
  const [deleting, setDeleting] = useState<{ row: ConnectedRow; agents: AffectedAgent[] } | null>(null)
  const [deleteBusy, setDeleteBusy] = useState(false)

  const load = async () => {
    try {
      const [s, p, c, all] = await Promise.all([getCatalogue(), listProviders(), listCustomProviders(), listCatalogueProviders()])
      setStatus(s.data)
      setConnected(p.data || [])
      setCustoms(c.data || [])
      setCatalogue(all.data || [])
    } catch (err) {
      addToast('error', errorMessage(err, 'Failed to load providers'))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
  }, [])

  // Reachability is checked in the background; look again shortly after the first load.
  useEffect(() => {
    if (loading) return
    const t = setTimeout(() => {
      Promise.all([listProviders(), listCustomProviders()])
        .then(([p, c]) => {
          setConnected(p.data || [])
          setCustoms(c.data || [])
        })
        .catch(() => {})
    }, 2500)
    return () => clearTimeout(t)
  }, [loading])

  const changed = async () => {
    setPanel(null)
    await load()
    await invalidatePickers()
  }

  const byId = new Map(catalogue.map((p) => [p.id, p]))
  const rows: ConnectedRow[] = [
    ...connected.map((p) => ({ kind: 'provider' as const, provider: p, catalogue: byId.get(p.id) })),
    ...customs.map((c) => ({ kind: 'custom' as const, custom: c, catalogue: byId.get(c.id) })),
  ]
  const listed = new Set(rows.map(rowId))
  const notConnected = catalogue.filter((p) => !listed.has(p.id))

  const refresh = async () => {
    setRefreshing(true)
    try {
      const res = await refreshCatalogue()
      setStatus(res.data)
      addToast('success', 'Catalogue is up to date')
      await invalidatePickers()
      const all = await listCatalogueProviders()
      setCatalogue(all.data || [])
    } catch (err) {
      addToast('error', errorMessage(err, 'Catalogue refresh failed'))
      getCatalogue().then((s) => setStatus(s.data)).catch(() => {})
    } finally {
      setRefreshing(false)
    }
  }

  const startDelete = async (row: ConnectedRow) => {
    const used = row.kind === 'custom' ? row.custom.used_by : row.provider.used_by
    const total = Object.values(used).reduce((a, b) => a + b, 0)
    if (!total) {
      setDeleting({ row, agents: [] })
      return
    }
    // The server names the agents in its 409.
    try {
      if (row.kind === 'custom') await deleteCustomProvider(row.custom.id)
      else await deleteProvider(row.provider.id)
      await changed()
    } catch (err) {
      const data = (err as { response?: { status?: number; data?: { data?: { agents?: AffectedAgent[] } } } }).response
      if (data?.status === 409) setDeleting({ row, agents: data.data?.data?.agents || [] })
      else addToast('error', errorMessage(err, 'Could not remove the provider'))
    }
  }

  const confirmDelete = async () => {
    if (!deleting) return
    setDeleteBusy(true)
    try {
      const res =
        deleting.row.kind === 'custom'
          ? await deleteCustomProvider(deleting.row.custom.id, true)
          : await deleteProvider(deleting.row.provider.id, true)
      const note = typeof res?.data === 'string' && res.data !== 'deleted' ? res.data : null
      addToast(note ? 'warning' : 'success', note ?? `${rowName(deleting.row)} removed`)
      setDeleting(null)
      await changed()
    } catch (err) {
      addToast('error', errorMessage(err, 'Could not remove the provider'))
    } finally {
      setDeleteBusy(false)
    }
  }

  const duplicate = (row: ConnectedRow) => {
    if (row.kind === 'custom') {
      const c = row.custom
      setPanel({ kind: 'custom', initial: { name: `${c.name} copy`, protocol: c.protocol, base_url: c.base_url, models: c.models.map((m) => ({ ...m, edited: false })) } })
    } else {
      const cat = row.catalogue
      setPanel({
        kind: 'custom',
        initial: {
          name: `${row.provider.name} (custom)`,
          protocol: cat?.dialect === 'anthropic' ? 'anthropic' : 'openai_compatible',
          base_url: cat?.endpoint?.includes('${') ? '' : (cat?.endpoint ?? ''),
        },
      })
    }
  }

  const refetch = async (row: ConnectedRow) => {
    if (row.kind !== 'custom') return
    try {
      const res = await fetchCustomProviderModels(row.custom.id)
      addToast('success', `${res.data.models.length} models for ${row.custom.name}`)
      await changed()
    } catch (err) {
      addToast('error', errorMessage(err, 'Could not reach the server'))
    }
  }

  const edit = (row: ConnectedRow) => {
    if (row.kind === 'custom') setPanel({ kind: 'custom', existing: row.custom })
    else if (row.catalogue) setPanel({ kind: 'connect', provider: row.catalogue, existing: row.provider })
  }

  const asOf = status ? new Date(status.fetched_at).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }) : null

  return (
    <div style={{ maxWidth: '760px', display: 'flex', flexDirection: 'column', gap: '1.25rem' }}>
      <div>
        <h2 style={{ marginBottom: '0.5rem' }}>Providers</h2>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', margin: 0 }}>
          Credentials are shared by every agent. Keys found in the server's environment are used too.
        </p>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', flexWrap: 'wrap', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
        <span style={{ flex: 1 }}>
          {status ? (
            <>
              Catalogue: <a href="https://models.dev" target="_blank" rel="noreferrer" style={{ color: 'var(--accent-primary)' }}>models.dev</a> · as of {asOf} ({status.source === 'bundled' ? 'bundled with this release' : 'updated'})
              {' · '}
              {status.provider_count} providers, {status.model_count.toLocaleString()} models
              {status.refresh.in_progress && ' · checking for updates…'}
              {status.refresh.last_error && (
                <div style={{ ...hintStyle, color: 'var(--error, #ef4444)' }}>Last update failed: {status.refresh.last_error}</div>
              )}
            </>
          ) : (
            'Loading catalogue…'
          )}
        </span>
        <button
          type="button"
          style={secondaryButton}
          disabled={refreshing || !status?.refresh.enabled}
          title={status && !status.refresh.enabled ? 'Catalogue refresh is turned off in the config (catalogue.refresh: false)' : undefined}
          onClick={refresh}
        >
          {refreshing ? 'Refreshing…' : 'Refresh now'}
        </button>
      </div>

      <section style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
        <h3 style={{ fontSize: '0.95rem', margin: 0 }}>Connected</h3>
        {loading ? (
          <p style={{ color: 'var(--text-tertiary)' }}>Loading…</p>
        ) : (
          <ConnectedList
            rows={rows}
            onViewModels={(row) => setDrawer({ id: rowId(row), name: rowName(row) })}
            onEdit={edit}
            onDuplicate={duplicate}
            onDelete={startDelete}
            onRefetch={refetch}
          />
        )}
      </section>

      <section style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
        <h3 style={{ fontSize: '0.95rem', margin: 0 }}>Add a provider</h3>
        <AddProvider
          providers={notConnected}
          onConnect={(provider) => setPanel({ kind: 'connect', provider })}
          onCustom={(name) => setPanel({ kind: 'custom', initial: name ? { name } : undefined })}
        />
      </section>

      <SlideOver
        open={panel !== null}
        onClose={() => setPanel(null)}
        title={
          panel?.kind === 'connect'
            ? `${panel.existing ? 'Edit' : 'Connect'} ${panel.provider.name}`
            : panel?.existing
              ? `Edit ${panel.existing.name}`
              : 'Custom provider'
        }
      >
        {panel?.kind === 'connect' && (
          <ConnectForm
            key={panel.provider.id}
            provider={panel.provider}
            existing={panel.existing}
            onSaved={() => {
              addToast('success', `${panel.provider.name} saved`)
              changed()
            }}
            onCancel={() => setPanel(null)}
          />
        )}
        {panel?.kind === 'custom' && (
          <CustomProviderForm
            key={panel.existing?.id ?? 'new'}
            existing={panel.existing}
            initial={panel.initial}
            onSaved={(p) => {
              addToast('success', `${p.name} saved`)
              changed()
            }}
            onCancel={() => setPanel(null)}
          />
        )}
      </SlideOver>

      <ModelsDrawer provider={drawer?.id ?? null} name={drawer?.name} onClose={() => setDrawer(null)} />

      {deleting && (
        <DeleteWarning
          name={rowName(deleting.row)}
          agents={deleting.agents}
          busy={deleteBusy}
          onConfirm={confirmDelete}
          onCancel={() => setDeleting(null)}
        />
      )}
    </div>
  )
}
