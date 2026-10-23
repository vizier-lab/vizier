import { useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { FaCheck, FaMagnifyingGlass, FaPlus } from 'react-icons/fa6'
import SlideOver from '../components/SlideOver'
import {
  ConfirmButton,
  CreatorLine,
  DetailTabs,
  IntegrationsPage,
  KindIcon,
  LegacyLinkedNotice,
  SecretInput,
  StatusBadge,
  inputStyle,
  labelStyle,
  noticeStyle,
  smallButton,
} from '../components/integrations'
import { apiError, useIntegrationStore, useIntegrations } from '../hooks/integrationStore'
import { useToastStore } from '../hooks/toastStore'
import type { IntegrationItem, KindListing } from '../interfaces/types'

export default function IntegrationsAddons() {
  const { agentId } = useParams()
  const { items, kinds, canManage, loading } = useIntegrations(agentId, 'addon')
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<string | null>(null)

  const shown = kinds.filter((k) =>
    `${k.name} ${k.description}`.toLowerCase().includes(query.trim().toLowerCase())
  )
  const installed = (kind: string) => items.some((i) => i.kind === kind)
  const selectedKind = kinds.find((k) => k.id === selected) ?? null

  return (
    <IntegrationsPage
      title="Addons"
      readOnly={!canManage}
      actions={
        <div style={{ position: 'relative', width: 240, maxWidth: '45vw' }}>
          <FaMagnifyingGlass size={12} style={{ position: 'absolute', left: 10, top: 11, color: 'var(--text-tertiary)' }} />
          <input style={{ ...inputStyle, paddingLeft: 28 }} placeholder="Search addons" value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
      }
    >
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: '1rem' }}>
        {shown.map((kind) => (
          <button
            key={kind.id}
            className="card"
            onClick={() => setSelected(kind.id)}
            style={{ padding: '1rem', textAlign: 'left', display: 'flex', flexDirection: 'column', gap: '0.6rem', cursor: 'pointer', color: 'var(--text-primary)' }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
              <KindIcon kind={kind.id} size={40} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontWeight: 600 }}>{kind.name}</div>
              </div>
              {installed(kind.id) ? (
                <span style={{ ...badge, background: 'rgba(16,185,129,0.12)', color: '#10b981' }}>
                  <FaCheck size={10} /> Installed
                </span>
              ) : (
                <span style={{ ...badge, background: 'var(--hover)', color: 'var(--accent-primary)' }}>Get</span>
              )}
            </div>
            <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>{kind.description}</div>
          </button>
        ))}
        <div
          className="card-static"
          style={{ padding: '1rem', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '0.5rem', borderStyle: 'dashed', color: 'var(--text-tertiary)', fontSize: '0.8rem' }}
        >
          <FaPlus size={10} /> More addons are on the way
        </div>
      </div>
      {shown.length === 0 && !loading && (
        <div style={{ color: 'var(--text-tertiary)', fontSize: '0.85rem' }}>No addon matches “{query}”.</div>
      )}

      <SlideOver open={selectedKind !== null} onClose={() => setSelected(null)} title={selectedKind?.name ?? ''}>
        {selectedKind && (
          <DetailTabs agentId={agentId ?? ''} integrationId={items.find((i) => i.kind === selectedKind.id)?.id}>
            <AddonDetails
              kind={selectedKind}
              config={items.find((i) => i.kind === selectedKind.id) ?? null}
              canManage={canManage}
            />
          </DetailTabs>
        )}
      </SlideOver>
    </IntegrationsPage>
  )
}

const badge: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 4,
  padding: '2px 10px',
  borderRadius: 999,
  fontSize: '0.72rem',
  fontWeight: 600,
}

/** Install, configure and uninstall one addon on the page's agent, which has at most one
 *  configuration of it. */
function AddonDetails({ kind, config, canManage }: { kind: KindListing; config: IntegrationItem | null; canManage: boolean }) {
  const store = useIntegrationStore()
  const addToast = useToastStore((s) => s.addToast)

  const defaults = useMemo(
    () => Object.fromEntries(kind.fields.filter((f) => f.default !== undefined).map((f) => [f.key, f.default])),
    [kind]
  )
  // Edits to send; a secret is present only once replaced.
  const [edits, setEdits] = useState<Record<string, unknown>>({})
  const [name, setName] = useState('')
  const [showForm, setShowForm] = useState(false)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    setEdits({})
    setName(config?.name ?? kind.name)
    setShowForm(false)
  }, [config?.id, kind])
  useEffect(() => {
    if (config?.missing?.length) setShowForm(true)
  }, [config?.missing?.length])

  const settings = { ...defaults, ...(config?.settings ?? {}), ...edits }
  const editable = config ? config.can_edit : canManage
  const formOpen = showForm || config !== null
  const missingNow = kind.fields
    .filter((f) => f.required)
    .filter((f) => {
      const v = settings[f.key]
      return v === undefined || v === null || v === ''
    })
    .map((f) => f.label)

  const run = async (what: () => Promise<void>, done: string) => {
    setBusy(true)
    try {
      await what()
      addToast('success', done, 'Applies from the agent’s next turn.')
    } catch (err) {
      const { message } = apiError(err)
      addToast('error', message)
    } finally {
      setBusy(false)
    }
  }

  /** Install (create the agent's configuration) or save changes to it. */
  const save = () =>
    run(async () => {
      if (!config) {
        await store.create({ kind: kind.id, name: name.trim() || kind.name, settings })
        return
      }
      await store.update(config.id, { name: name.trim() || config.name, settings: { ...config.settings, ...edits } })
      setEdits({})
    }, config ? `${kind.name} saved` : `${kind.name} installed`)

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
        <KindIcon kind={kind.id} size={48} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>{kind.description}</div>
          {config && (
            <div style={{ marginTop: 4, display: 'flex', flexDirection: 'column', gap: 2 }}>
              <StatusBadge item={config} runningLabel="Ready" />
              <CreatorLine item={config} />
            </div>
          )}
        </div>
      </div>

      {!config && canManage && !showForm && (
        <button className="btn btn-primary" style={smallButton} onClick={() => setShowForm(true)}>
          Install on this agent
        </button>
      )}
      {!config && !canManage && <div style={noticeStyle}>You can’t install addons on this agent.</div>}

      {config && <LegacyLinkedNotice item={config} />}

      {(formOpen || config) && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
          <div style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-primary)' }}>Settings</div>
          <section>
            <label style={labelStyle}>Name</label>
            <input style={inputStyle} value={name} disabled={!editable} onChange={(e) => setName(e.target.value)} />
          </section>
          {kind.fields.map((field) => (
            <section key={field.key}>
              {field.type === 'bool' ? (
                <label style={{ display: 'flex', gap: '0.4rem', alignItems: 'center', fontSize: '0.85rem', cursor: 'pointer' }}>
                  <input type="checkbox" checked={Boolean(settings[field.key])} disabled={!editable}
                    onChange={(e) => setEdits((x) => ({ ...x, [field.key]: e.target.checked }))} />
                  {field.label}
                </label>
              ) : (
                <>
                  <label style={labelStyle}>{field.label}{field.required ? ' *' : ''}</label>
                  {field.type === 'secret' ? (
                    <SecretInput
                      stored={config?.settings[field.key]}
                      value={edits[field.key] as string | undefined}
                      disabled={!editable}
                      onChange={(v) =>
                        setEdits((x) => {
                          const next = { ...x }
                          if (v === undefined) delete next[field.key]
                          else next[field.key] = v
                          return next
                        })
                      }
                    />
                  ) : (
                    <input style={inputStyle} value={String(settings[field.key] ?? '')} disabled={!editable}
                      onChange={(e) => setEdits((x) => ({ ...x, [field.key]: e.target.value }))} />
                  )}
                </>
              )}
            </section>
          ))}
          {editable && (
            <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
              {config && (
                <ConfirmButton
                  label={config.legacy_linked ? 'Unlink from this agent' : 'Uninstall'}
                  confirmLabel={config.legacy_linked ? 'Unlink' : 'Uninstall'}
                  prompt={config.legacy_linked ? undefined : 'This deletes its settings. Uninstall?'}
                  disabled={busy}
                  onConfirm={() =>
                    run(() => store.remove(config.id), `${kind.name} ${config.legacy_linked ? 'unlinked' : 'uninstalled'}`)
                  }
                />
              )}
              <div style={{ flex: 1 }} />
              <button className="btn btn-primary" style={smallButton} disabled={busy || missingNow.length > 0}
                title={missingNow.length ? `Set ${missingNow.join(', ')} first` : undefined}
                onClick={save}>
                {config ? 'Save' : 'Install'}
              </button>
            </div>
          )}
        </div>
      )}

    </div>
  )
}
