import { useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router'
import { FaCheck, FaMagnifyingGlass, FaPlus } from 'react-icons/fa6'
import SlideOver from '../components/SlideOver'
import {
  IntegrationsPage,
  KindIcon,
  SecretInput,
  SharedEditWarning,
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
  const installed = (kind: string) => items.some((i) => i.kind === kind && i.attached)
  const selectedKind = kinds.find((k) => k.id === selected) ?? null

  return (
    <IntegrationsPage
      title="Addons"
      subtitle="Extra tools for this agent. An addon’s settings are shared; installing it on an agent is per agent."
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
          <AddonDetails
            kind={selectedKind}
            configs={items.filter((i) => i.kind === selectedKind.id)}
            canManage={canManage}
          />
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

/** Install, uninstall and the shared settings of one addon for the page's agent. */
function AddonDetails({ kind, configs, canManage }: { kind: KindListing; configs: IntegrationItem[]; canManage: boolean }) {
  const store = useIntegrationStore()
  const addToast = useToastStore((s) => s.addToast)
  const attached = configs.find((c) => c.attached)
  // Which configuration the panel shows: the installed one, else the first, else a new one.
  const [chosen, setChosen] = useState<string>(attached?.id ?? configs[0]?.id ?? 'new')
  useEffect(() => {
    if (chosen !== 'new' && !configs.some((c) => c.id === chosen)) setChosen(attached?.id ?? configs[0]?.id ?? 'new')
  }, [configs, chosen, attached])
  const config = configs.find((c) => c.id === chosen) ?? null

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

  const settings = { ...defaults, ...(config?.settings ?? {}), ...edits }
  const editable = config ? config.can_edit : canManage
  const formOpen = showForm || chosen === 'new' || (config?.missing?.length ?? 0) > 0
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
      const { message, data } = apiError(err)
      if (data?.error === 'incomplete') setShowForm(true)
      addToast('error', message)
    } finally {
      setBusy(false)
    }
  }

  /** Save the form (a new configuration is created and installed in one step). */
  const saveAndInstall = (install: boolean) =>
    run(async () => {
      if (!config) {
        await store.create({ kind: kind.id, name: name.trim() || kind.name, settings })
        return
      }
      if (Object.keys(edits).length > 0 || name.trim() !== config.name) {
        await store.update(config.id, { name: name.trim() || config.name, settings: { ...config.settings, ...edits } })
      }
      if (install && !config.attached) await store.attach(config.id)
      setEdits({})
    }, install ? `${kind.name} installed` : `${kind.name} saved`)

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
        <KindIcon kind={kind.id} size={48} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>{kind.description}</div>
          {config && <div style={{ marginTop: 4 }}><StatusBadge item={config} runningLabel="Ready" /></div>}
        </div>
      </div>

      {configs.length > 1 || (configs.length > 0 && canManage) ? (
        <section>
          <label style={labelStyle}>Configuration</label>
          <select style={inputStyle} value={chosen} onChange={(e) => setChosen(e.target.value)}>
            {configs.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}{c.attached ? ' (installed here)' : ''}
              </option>
            ))}
            {canManage && <option value="new">New configuration…</option>}
          </select>
        </section>
      ) : null}

      {config && config.attached && config.can_attach && (
        <button className="btn btn-secondary" style={smallButton} disabled={busy}
          onClick={() => run(() => store.detach(config.id), `${kind.name} uninstalled from this agent`)}>
          Uninstall from this agent
        </button>
      )}
      {config && !config.attached && config.can_attach && !formOpen && (
        <button className="btn btn-primary" style={smallButton} disabled={busy}
          onClick={() => run(() => store.attach(config.id), `${kind.name} installed`)}>
          Install on this agent
        </button>
      )}
      {!config && !canManage && <div style={noticeStyle}>You can’t install addons on this agent.</div>}

      {config && <SharedEditWarning item={config} />}

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
                <button className="btn btn-secondary" style={smallButton} disabled={busy} onClick={() => saveAndInstall(false)}>
                  Save
                </button>
              )}
              {(!config || (!config.attached && config.can_attach)) && (
                <button className="btn btn-primary" style={smallButton} disabled={busy || missingNow.length > 0}
                  title={missingNow.length ? `Set ${missingNow.join(', ')} first` : undefined}
                  onClick={() => saveAndInstall(true)}>
                  Save and install
                </button>
              )}
            </div>
          )}
        </div>
      )}

    </div>
  )
}
