import { useState } from 'react'
import { useParams } from 'react-router'
import { FaPlus, FaTrash } from 'react-icons/fa6'
import SlideOver from '../components/SlideOver'
import {
  ConfirmButton,
  IntegrationsPage,
  SecretInput,
  SharedEditWarning,
  StatusBadge,
  Switch,
  inputStyle,
  labelStyle,
  noticeStyle,
  smallButton,
  statusError,
} from '../components/integrations'
import { apiError, useIntegrations } from '../hooks/integrationStore'
import { useToastStore } from '../hooks/toastStore'
import type { IntegrationItem } from '../interfaces/types'

interface EnvRow {
  key: string
  /** The stored, redacted value; empty for a new row. */
  stored?: string
  /** A replaced value; `undefined` keeps `stored`. */
  value?: string
}

interface McpForm {
  name: string
  transport: 'local' | 'http'
  command: string
  args: string
  uri: string
  env: EnvRow[]
}

const EMPTY_FORM: McpForm = { name: '', transport: 'local', command: '', args: '', uri: '', env: [] }

function formFrom(item: IntegrationItem): McpForm {
  const s = item.settings as {
    transport?: string
    command?: string
    args?: string[]
    uri?: string
    env?: Record<string, string>
  }
  return {
    name: item.name,
    transport: s.transport === 'http' ? 'http' : 'local',
    command: s.command ?? '',
    args: (s.args ?? []).join('\n'),
    uri: s.uri ?? '',
    env: Object.entries(s.env ?? {}).map(([key, stored]) => ({ key, stored })),
  }
}

/** Settings to send: a redacted env value goes back unchanged, which the server keeps. */
function settingsFrom(form: McpForm): Record<string, unknown> {
  if (form.transport === 'http') return { transport: 'http', uri: form.uri.trim() }
  const env = Object.fromEntries(
    form.env
      .filter((row) => row.key.trim())
      .map((row) => [row.key.trim(), row.value ?? row.stored ?? ''])
  )
  return {
    transport: 'local',
    command: form.command.trim(),
    args: form.args.split('\n').map((a) => a.trim()).filter(Boolean),
    env,
  }
}

export default function IntegrationsMcp() {
  const { agentId } = useParams()
  const addToast = useToastStore((s) => s.addToast)
  const store = useIntegrations(agentId, 'mcp')
  const { items, canManage, loading } = store

  // `null`: closed; `'new'`: the add form; otherwise the integration being edited.
  const [editing, setEditing] = useState<IntegrationItem | 'new' | null>(null)
  const [form, setForm] = useState<McpForm>(EMPTY_FORM)
  const [saving, setSaving] = useState(false)

  const open = (target: IntegrationItem | 'new') => {
    setEditing(target)
    setForm(target === 'new' ? EMPTY_FORM : formFrom(target))
  }
  const item = editing !== 'new' ? editing : null
  const readOnly = item ? !item.can_edit : false

  const conflictToast = (err: unknown) => {
    const { message, data } = apiError(err)
    addToast('error', message, data?.error === 'name_in_use_on_agent' ? 'Rename one of them first.' : undefined)
  }

  const toggle = async (target: IntegrationItem, on: boolean) => {
    try {
      if (on) await store.attach(target.id)
      else await store.detach(target.id)
      addToast('success', `${target.name} ${on ? 'attached' : 'detached'}`, 'Applies from the agent’s next turn.')
    } catch (err) {
      conflictToast(err)
    }
  }

  const save = async () => {
    setSaving(true)
    try {
      if (editing === 'new') {
        await store.create({ kind: 'mcp', name: form.name.trim(), settings: settingsFrom(form) })
        addToast('success', `${form.name} added`, 'Applies from the agent’s next turn.')
      } else if (item) {
        await store.update(item.id, { name: form.name.trim(), settings: settingsFrom(form) })
        addToast('success', `${form.name} saved`, 'Applies from the next turn of every agent using it.')
      }
      setEditing(null)
    } catch (err) {
      conflictToast(err)
    } finally {
      setSaving(false)
    }
  }

  const remove = async (target: IntegrationItem) => {
    try {
      await store.remove(target.id, true)
      addToast('success', `${target.name} deleted`)
      setEditing(null)
    } catch (err) {
      addToast('error', apiError(err).message)
    }
  }

  const setEnv = (i: number, row: Partial<EnvRow>) =>
    setForm((f) => ({ ...f, env: f.env.map((r, j) => (j === i ? { ...r, ...row } : r)) }))

  return (
    <IntegrationsPage
      title="MCP servers"
      subtitle="Model Context Protocol servers this agent can call from its scripts. Each agent keeps its own connection."
      readOnly={!canManage}
      actions={
        canManage && (
          <button className="btn btn-primary" style={smallButton} onClick={() => open('new')}>
            <FaPlus size={12} /> Add server
          </button>
        )
      }
    >
      <div className="card-static" style={{ padding: 0, overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.85rem' }}>
          <thead>
            <tr style={{ textAlign: 'left', color: 'var(--text-tertiary)', fontSize: '0.75rem' }}>
              <th style={th}>Server</th>
              <th style={th}>Status</th>
              <th style={{ ...th, textAlign: 'right' }}>On for this agent</th>
            </tr>
          </thead>
          <tbody>
            {items.map((i) => (
              <tr key={i.id} onClick={() => open(i)} style={{ borderTop: '1px solid var(--border)', cursor: 'pointer' }}>
                <td style={td}>
                  <div style={{ fontWeight: 600 }}>{i.name}</div>
                  <div style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)', fontFamily: 'monospace' }}>
                    {i.settings.transport === 'http' ? String(i.settings.uri ?? '') : String(i.settings.command ?? '')}
                  </div>
                </td>
                <td style={td}>
                  <StatusBadge item={i} runningLabel="Connected" />
                  {i.attached && statusError(i) && (
                    <div style={{ fontSize: '0.72rem', color: '#ef4444', marginTop: 2 }}>{statusError(i)}</div>
                  )}
                </td>
                <td style={{ ...td, textAlign: 'right' }}>
                  <Switch
                    checked={i.attached}
                    disabled={!i.can_attach}
                    title={i.can_attach ? undefined : 'You can’t change this agent’s servers'}
                    onChange={(on) => toggle(i, on)}
                  />
                </td>
              </tr>
            ))}
            {items.length === 0 && (
              <tr>
                <td colSpan={3} style={{ ...td, textAlign: 'center', color: 'var(--text-tertiary)', padding: '2rem' }}>
                  {loading ? 'Loading…' : 'No MCP servers yet.'}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <SlideOver
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === 'new' ? 'Add MCP server' : item?.name ?? ''}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
          {item && <SharedEditWarning item={item} />}
          {readOnly && <div style={noticeStyle}>Only its creator or an integrations manager can edit this server.</div>}

          <section>
            <label style={labelStyle}>Name</label>
            <input
              style={inputStyle}
              value={form.name}
              disabled={readOnly}
              placeholder="github"
              onChange={(e) => setForm({ ...form, name: e.target.value.toLowerCase() })}
            />
            <div style={{ fontSize: '0.72rem', color: 'var(--text-tertiary)', marginTop: 4 }}>
              Its tools are named <code>{form.name || 'name'}_&lt;tool&gt;</code>. Lowercase letters, digits, - and _.
            </div>
          </section>

          <section style={{ display: 'flex', gap: '1rem', fontSize: '0.85rem' }}>
            {(['local', 'http'] as const).map((t) => (
              <label key={t} style={{ display: 'flex', gap: '0.35rem', alignItems: 'center', cursor: 'pointer' }}>
                <input
                  type="radio"
                  name="mcp-transport"
                  checked={form.transport === t}
                  disabled={readOnly}
                  onChange={() => setForm({ ...form, transport: t })}
                />
                {t === 'local' ? 'Local command' : 'HTTP'}
              </label>
            ))}
          </section>

          {form.transport === 'local' ? (
            <>
              <section>
                <label style={labelStyle}>Command</label>
                <input style={inputStyle} value={form.command} disabled={readOnly} placeholder="npx"
                  onChange={(e) => setForm({ ...form, command: e.target.value })} />
              </section>
              <section>
                <label style={labelStyle}>Arguments (one per line)</label>
                <textarea style={{ ...inputStyle, minHeight: 80, fontFamily: 'monospace' }} value={form.args} disabled={readOnly}
                  placeholder={'-y\n@modelcontextprotocol/server-github'}
                  onChange={(e) => setForm({ ...form, args: e.target.value })} />
              </section>
              <section>
                <label style={labelStyle}>Environment</label>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  {form.env.map((row, i) => (
                    <div key={i} style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                      <input style={{ ...inputStyle, width: '40%', fontFamily: 'monospace' }} value={row.key} disabled={readOnly}
                        placeholder="GITHUB_TOKEN" onChange={(e) => setEnv(i, { key: e.target.value })} />
                      <div style={{ flex: 1 }}>
                        <SecretInput stored={row.stored} value={row.value} disabled={readOnly}
                          onChange={(value) => setEnv(i, { value })} />
                      </div>
                      {!readOnly && (
                        <button type="button" className="btn btn-ghost" style={smallButton} title="Remove"
                          onClick={() => setForm((f) => ({ ...f, env: f.env.filter((_, j) => j !== i) }))}>
                          <FaTrash size={12} />
                        </button>
                      )}
                    </div>
                  ))}
                  {!readOnly && (
                    <button type="button" className="btn btn-secondary" style={{ ...smallButton, alignSelf: 'flex-start' }}
                      onClick={() => setForm((f) => ({ ...f, env: [...f.env, { key: '' }] }))}>
                      <FaPlus size={10} /> Variable
                    </button>
                  )}
                </div>
              </section>
            </>
          ) : (
            <section>
              <label style={labelStyle}>URL</label>
              <input style={inputStyle} value={form.uri} disabled={readOnly} placeholder="https://example.com/mcp"
                onChange={(e) => setForm({ ...form, uri: e.target.value })} />
            </section>
          )}

          {!readOnly && (
            <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center', paddingTop: '0.5rem' }}>
              {item && (
                <ConfirmButton
                  label="Delete"
                  confirmLabel="Delete"
                  prompt={item.attached || item.also_used_by.length || item.others ? 'It is in use. Delete anyway?' : 'Delete?'}
                  onConfirm={() => remove(item)}
                />
              )}
              <div style={{ flex: 1 }} />
              <button className="btn btn-secondary" style={smallButton} onClick={() => setEditing(null)}>Cancel</button>
              <button className="btn btn-primary" style={smallButton} disabled={saving || !form.name.trim()} onClick={save}>
                {editing === 'new' ? 'Add server' : 'Save'}
              </button>
            </div>
          )}
        </div>
      </SlideOver>
    </IntegrationsPage>
  )
}

const th: React.CSSProperties = { padding: '0.6rem 1rem', fontWeight: 600 }
const td: React.CSSProperties = { padding: '0.7rem 1rem', verticalAlign: 'top' }
