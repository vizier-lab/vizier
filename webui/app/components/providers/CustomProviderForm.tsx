import { useEffect, useState } from 'react'
import {
  createCustomProvider,
  fetchCustomProviderModels,
  listCatalogueModels,
  testCustomProvider,
  updateCustomProvider,
} from '../../services/vizier'
import type {
  CatalogueModel,
  CustomModel,
  CustomProtocol,
  CustomProviderBody,
  CustomProviderResponse,
  ProviderTestResult,
} from '../../interfaces/types'
import {
  badgeStyle,
  cardStyle,
  errorMessage,
  hintStyle,
  inputStyle,
  labelStyle,
  primaryButton,
  secondaryButton,
} from './styles'

interface CustomProviderFormProps {
  /** Editing a saved provider: the id is fixed and secrets are write-only. */
  existing?: CustomProviderResponse
  /** Prefill for a new provider (Duplicate, or "Create custom provider …" from a search). */
  initial?: Partial<CustomProviderBody>
  onSaved: (provider: CustomProviderResponse) => void
  onCancel: () => void
}

/** `name` as an id, the way the server slugs it. */
function slug(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64)
}

const PROTOCOLS: { value: CustomProtocol; title: string; body: string; path: string }[] = [
  {
    value: 'openai_compatible',
    title: 'OpenAI-compatible',
    body: 'vLLM, LiteLLM, LM Studio, llama.cpp server, most proxies',
    path: '/chat/completions',
  },
  {
    value: 'anthropic',
    title: 'Anthropic',
    body: 'Anthropic Messages API and compatible gateways',
    path: '/v1/messages',
  },
]

type Tri = boolean | null | undefined

function TriSelect({ value, onChange, filled }: { value: Tri; onChange: (v: boolean | null) => void; filled?: boolean | null }) {
  const show = value ?? null
  return (
    <select
      style={{ ...inputStyle, padding: '0.25rem 0.35rem', fontSize: '0.78rem' }}
      value={show === null ? '' : show ? 'yes' : 'no'}
      onChange={(e) => onChange(e.target.value === '' ? null : e.target.value === 'yes')}
      title={show === null && filled != null ? 'From the catalogue' : undefined}
    >
      <option value="">{filled != null ? `${filled ? 'yes' : 'no'} ⓒ` : '?'}</option>
      <option value="yes">yes</option>
      <option value="no">no</option>
    </select>
  )
}

/** A custom provider in two steps: the connection, then its models (FR-022–FR-025). */
export default function CustomProviderForm({ existing, initial, onSaved, onCancel }: CustomProviderFormProps) {
  const editing = !!existing
  const [step, setStep] = useState<1 | 2>(1)
  const [name, setName] = useState(existing?.name ?? initial?.name ?? '')
  const [id, setId] = useState(existing?.id ?? initial?.id ?? '')
  const [idTouched, setIdTouched] = useState(editing || !!initial?.id)
  const [protocol, setProtocol] = useState<CustomProtocol>(existing?.protocol ?? initial?.protocol ?? 'openai_compatible')
  const [baseUrl, setBaseUrl] = useState(existing?.base_url ?? initial?.base_url ?? '')
  const [apiKey, setApiKey] = useState('')
  const [replaceKey, setReplaceKey] = useState(!existing?.has_api_key)
  // Header values are write-only: a saved header shows as •••• until replaced.
  const [headers, setHeaders] = useState<{ name: string; value: string; saved: boolean; removed?: boolean }[]>(
    (existing?.headers ?? []).map((h) => ({ name: h, value: '', saved: true }))
  )
  const [headersOpen, setHeadersOpen] = useState((existing?.headers.length ?? 0) > 0)
  const [models, setModels] = useState<CustomModel[]>(existing?.models ?? initial?.models ?? [])
  const [filled, setFilled] = useState<Record<string, CatalogueModel>>({})
  const [saved, setSaved] = useState<CustomProviderResponse | undefined>(existing)
  const [busy, setBusy] = useState<string | null>(null)
  const [test, setTest] = useState<ProviderTestResult | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!idTouched) setId(slug(name))
  }, [name, idTouched])

  // What the catalogue fills in for each row (marked ⓒ), once the provider exists.
  const loadFilled = async (providerId: string) => {
    try {
      const res = await listCatalogueModels(providerId, true)
      setFilled(Object.fromEntries((res.data || []).map((m) => [m.id, m])))
    } catch {
      setFilled({})
    }
  }
  useEffect(() => {
    if (saved) loadFilled(saved.id)
  }, [saved?.id])

  const body = (withModels: boolean): CustomProviderBody => {
    const hdrs: Record<string, string | null> = {}
    for (const h of headers) {
      if (!h.name.trim()) continue
      if (h.removed) hdrs[h.name.trim()] = null
      else if (h.value) hdrs[h.name.trim()] = h.value
      else if (h.saved) hdrs[h.name.trim()] = ''
    }
    return {
      id: id || undefined,
      name,
      protocol,
      base_url: baseUrl.trim(),
      api_key: replaceKey ? apiKey : undefined,
      headers: hdrs,
      models: withModels ? models : undefined,
    }
  }

  const run = async (label: string, f: () => Promise<void>) => {
    setBusy(label)
    setError(null)
    try {
      await f()
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setBusy(null)
    }
  }

  const runTest = () =>
    run('test', async () => {
      setTest(null)
      const res = await testCustomProvider(body(false))
      setTest(res.data)
    })

  const saveConnection = () =>
    run('save', async () => {
      const res = saved ? await updateCustomProvider(saved.id, body(false)) : await createCustomProvider(body(false))
      setSaved(res.data)
      setModels(res.data.models)
      setStep(2)
    })

  const fetchModels = () =>
    run('fetch', async () => {
      if (!saved) return
      // Save edits first so the merge keeps them.
      await updateCustomProvider(saved.id, body(true))
      const res = await fetchCustomProviderModels(saved.id)
      setSaved(res.data)
      setModels(res.data.models)
      await loadFilled(saved.id)
    })

  const saveAll = () =>
    run('save', async () => {
      const res = saved ? await updateCustomProvider(saved.id, body(true)) : await createCustomProvider(body(true))
      onSaved(res.data)
    })

  const setRow = (i: number, patch: Partial<CustomModel>) =>
    setModels((rows) => rows.map((r, j) => (j === i ? { ...r, ...patch } : r)))

  const effective = (row: CustomModel, key: 'tool_call' | 'image_input' | 'reasoning') => {
    const own = row[key]
    if (own != null) return own
    const f = filled[row.id]?.caps[key]
    return f ?? null
  }
  const protocolInfo = PROTOCOLS.find((p) => p.value === protocol)!
  const canSubmitConnection = name.trim() && id && /^https?:\/\/.+/.test(baseUrl.trim())

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <div style={{ display: 'flex', gap: '0.5rem', fontSize: '0.8rem' }}>
        {(['Connection', 'Models'] as const).map((label, i) => (
          <button
            key={label}
            type="button"
            disabled={i === 1 && !saved}
            onClick={() => setStep((i + 1) as 1 | 2)}
            style={{
              ...badgeStyle(),
              cursor: i === 1 && !saved ? 'not-allowed' : 'pointer',
              background: step === i + 1 ? 'var(--hover)' : 'transparent',
              color: 'var(--text-primary)',
              padding: '0.2rem 0.7rem',
            }}
          >
            {i + 1}. {label}
          </button>
        ))}
      </div>

      {step === 1 && (
        <>
          <section>
            <label style={labelStyle}>Name</label>
            <input style={inputStyle} value={name} placeholder="Home vLLM" onChange={(e) => setName(e.target.value)} />
          </section>
          <section>
            <label style={labelStyle}>Id</label>
            <input
              style={{ ...inputStyle, fontFamily: 'var(--font-mono)', opacity: editing ? 0.6 : 1 }}
              value={id}
              disabled={editing}
              onChange={(e) => {
                setIdTouched(true)
                setId(e.target.value.toLowerCase().replace(/[^a-z0-9._-]/g, '-'))
              }}
            />
            <div style={hintStyle}>{editing ? "A provider's id can't change once saved." : 'Agents refer to the provider by this id. It can’t be changed after saving.'}</div>
          </section>
          <section>
            <label style={labelStyle}>Protocol</label>
            <div style={{ display: 'flex', gap: '0.6rem', flexWrap: 'wrap' }}>
              {PROTOCOLS.map((p) => (
                <label
                  key={p.value}
                  style={{
                    ...cardStyle,
                    flex: '1 1 12rem',
                    padding: '0.6rem 0.75rem',
                    cursor: 'pointer',
                    borderColor: protocol === p.value ? 'var(--accent-primary)' : 'var(--border)',
                  }}
                >
                  <input type="radio" name="protocol" checked={protocol === p.value} onChange={() => setProtocol(p.value)} style={{ marginRight: '0.4rem' }} />
                  <strong style={{ fontSize: '0.85rem' }}>{p.title}</strong>
                  <div style={hintStyle}>{p.body}</div>
                </label>
              ))}
            </div>
          </section>
          <section>
            <label style={labelStyle}>Base URL</label>
            <input
              style={inputStyle}
              value={baseUrl}
              placeholder={protocol === 'anthropic' ? 'https://gateway.example.com' : 'http://10.0.0.5:8000/v1'}
              onChange={(e) => setBaseUrl(e.target.value)}
            />
            {baseUrl.trim() && (
              <div style={hintStyle}>
                Requests go to <code>{baseUrl.trim().replace(/\/+$/, '')}{protocolInfo.path}</code>
              </div>
            )}
          </section>
          <section>
            <label style={labelStyle}>API key</label>
            {replaceKey ? (
              <input style={inputStyle} type="password" autoComplete="off" value={apiKey} placeholder="Optional" onChange={(e) => setApiKey(e.target.value)} />
            ) : (
              <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                <span style={{ fontFamily: 'var(--font-mono)' }}>••••••••</span>
                <button type="button" style={secondaryButton} onClick={() => setReplaceKey(true)}>
                  Replace
                </button>
              </div>
            )}
          </section>
          <section>
            <button type="button" className="btn btn-ghost" style={{ fontSize: '0.8rem', padding: 0 }} onClick={() => setHeadersOpen((o) => !o)}>
              {headersOpen ? '▾' : '▸'} Extra headers
            </button>
            {headersOpen && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem', marginTop: '0.4rem' }}>
                {headers.map((h, i) =>
                  h.removed ? null : (
                    <div key={i} style={{ display: 'flex', gap: '0.4rem' }}>
                      <input
                        style={{ ...inputStyle, flex: 1, fontFamily: 'var(--font-mono)' }}
                        value={h.name}
                        disabled={h.saved}
                        placeholder="x-header-name"
                        onChange={(e) => setHeaders((hs) => hs.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))}
                      />
                      <input
                        style={{ ...inputStyle, flex: 1 }}
                        type="password"
                        value={h.value}
                        placeholder={h.saved ? '•••• (saved — type to replace)' : 'value'}
                        onChange={(e) => setHeaders((hs) => hs.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)))}
                      />
                      <button
                        type="button"
                        style={secondaryButton}
                        onClick={() =>
                          setHeaders((hs) => (h.saved ? hs.map((x, j) => (j === i ? { ...x, removed: true } : x)) : hs.filter((_, j) => j !== i)))
                        }
                      >
                        ✕
                      </button>
                    </div>
                  )
                )}
                <button type="button" className="btn btn-ghost" style={{ alignSelf: 'flex-start', fontSize: '0.8rem' }} onClick={() => setHeaders((hs) => [...hs, { name: '', value: '', saved: false }])}>
                  + Add header
                </button>
              </div>
            )}
          </section>

          {test && (
            <div style={badgeStyle(test.ok ? 'ok' : 'error')}>
              {test.ok
                ? `✓ Connected${test.model_count != null ? ` · ${test.model_count} models` : ''}`
                : `✗ ${test.status ? `${test.status} ` : ''}${test.error || 'Failed'}`}
            </div>
          )}
          {error && <div style={{ ...hintStyle, color: 'var(--error, #ef4444)' }}>{error}</div>}

          <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
            <button type="button" style={secondaryButton} onClick={onCancel}>
              Cancel
            </button>
            <button type="button" style={secondaryButton} disabled={!canSubmitConnection || busy !== null} onClick={runTest}>
              {busy === 'test' ? 'Testing…' : 'Test connection'}
            </button>
            <button type="button" style={primaryButton(!canSubmitConnection || busy !== null)} disabled={!canSubmitConnection || busy !== null} onClick={saveConnection}>
              {busy === 'save' ? 'Saving…' : saved ? 'Save & continue' : 'Create & continue'}
            </button>
          </div>
        </>
      )}

      {step === 2 && saved && (
        <>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
            <span style={{ flex: 1, fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
              {saved.models_fetched_at ? `Last fetched ${new Date(saved.models_fetched_at).toLocaleString()}` : 'Not fetched yet.'}{' '}
              Values marked ⓒ come from the catalogue; edited rows are kept when you fetch again.
            </span>
            <button type="button" style={secondaryButton} disabled={busy !== null} onClick={fetchModels}>
              {busy === 'fetch' ? 'Fetching…' : 'Fetch from server'}
            </button>
          </div>
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.8rem' }}>
              <thead>
                <tr style={{ color: 'var(--text-tertiary)', textAlign: 'left' }}>
                  <th style={{ padding: '0.3rem' }}>Model id</th>
                  <th style={{ padding: '0.3rem' }}>Context</th>
                  <th style={{ padding: '0.3rem' }}>Output</th>
                  <th style={{ padding: '0.3rem' }}>Tools</th>
                  <th style={{ padding: '0.3rem' }}>Vision</th>
                  <th style={{ padding: '0.3rem' }}>Reasoning</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {models.map((row, i) => {
                  const f = filled[row.id]
                  const fContext = f?.filled_from_catalogue?.includes('limit.context') ? f.limit.context : null
                  const fOutput = f?.filled_from_catalogue?.includes('limit.output') ? f.limit.output : null
                  const tools = effective(row, 'tool_call')
                  return (
                    <tr key={i} style={{ borderTop: '1px solid var(--border)' }}>
                      <td style={{ padding: '0.3rem', minWidth: '12rem' }}>
                        <input style={{ ...inputStyle, padding: '0.25rem 0.4rem', fontFamily: 'var(--font-mono)', fontSize: '0.78rem' }} value={row.id} onChange={(e) => setRow(i, { id: e.target.value })} />
                        {tools !== true && (
                          <div style={{ ...hintStyle, color: 'var(--warning, #d97706)' }}>
                            ⚠ tool calling {tools === false ? 'not supported' : 'unknown'}; agents need it
                          </div>
                        )}
                      </td>
                      <td style={{ padding: '0.3rem' }}>
                        <input
                          style={{ ...inputStyle, padding: '0.25rem 0.4rem', width: '6.5rem', fontSize: '0.78rem' }}
                          type="number"
                          value={row.context ?? ''}
                          placeholder={fContext ? `${fContext} ⓒ` : '?'}
                          onChange={(e) => setRow(i, { context: e.target.value ? parseInt(e.target.value) : null })}
                        />
                      </td>
                      <td style={{ padding: '0.3rem' }}>
                        <input
                          style={{ ...inputStyle, padding: '0.25rem 0.4rem', width: '6rem', fontSize: '0.78rem' }}
                          type="number"
                          value={row.output ?? ''}
                          placeholder={fOutput ? `${fOutput} ⓒ` : '?'}
                          onChange={(e) => setRow(i, { output: e.target.value ? parseInt(e.target.value) : null })}
                        />
                      </td>
                      <td style={{ padding: '0.3rem' }}>
                        <TriSelect value={row.tool_call} filled={f?.filled_from_catalogue?.includes('caps.tool_call') ? f.caps.tool_call : null} onChange={(v) => setRow(i, { tool_call: v })} />
                      </td>
                      <td style={{ padding: '0.3rem' }}>
                        <TriSelect value={row.image_input} filled={f?.filled_from_catalogue?.includes('caps.image_input') ? f.caps.image_input : null} onChange={(v) => setRow(i, { image_input: v })} />
                      </td>
                      <td style={{ padding: '0.3rem' }}>
                        <TriSelect value={row.reasoning} filled={f?.filled_from_catalogue?.includes('caps.reasoning') ? f.caps.reasoning : null} onChange={(v) => setRow(i, { reasoning: v })} />
                      </td>
                      <td style={{ padding: '0.3rem' }}>
                        <button type="button" style={{ ...secondaryButton, padding: '0.2rem 0.5rem' }} onClick={() => setModels((rows) => rows.filter((_, j) => j !== i))}>
                          ✕
                        </button>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          <button type="button" className="btn btn-ghost" style={{ alignSelf: 'flex-start', fontSize: '0.8rem' }} onClick={() => setModels((rows) => [...rows, { id: '' }])}>
            + Add model
          </button>
          {error && <div style={{ ...hintStyle, color: 'var(--error, #ef4444)' }}>{error}</div>}
          <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
            <button type="button" style={secondaryButton} onClick={onCancel}>
              Cancel
            </button>
            <button type="button" style={primaryButton(busy !== null)} disabled={busy !== null} onClick={saveAll}>
              {busy === 'save' ? 'Saving…' : 'Save'}
            </button>
          </div>
        </>
      )}
    </div>
  )
}
